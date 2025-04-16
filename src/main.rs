use std::{collections::HashMap, fs};

use anyhow::Result;
use proc_macro2::{Ident, TokenStream};
use quote::{format_ident, quote};
use tree_sitter::{Language, Node, Parser, Query, QueryCursor, StreamingIterator};

fn main() -> Result<()> {
    let mut state = State::new();

    let mut parser = Parser::new();
    let lang: Language = tree_sitter_c_sharp::LANGUAGE.into();
    parser
        .set_language(&lang)
        .expect("Error loading C# grammar");

    let files = fs::read_dir("files")?
        .filter_map(|f| {
            if let Ok(entry) = f {
                if entry.file_type().unwrap().is_file()
                    && entry.file_name().to_string_lossy().ends_with(".cs")
                {
                    return Some(entry.path());
                }
            }
            None
        })
        .filter_map(|f| {
            if let Ok(source_code) = fs::read_to_string(&f) {
                let tree = parser.parse(source_code.clone(), None)?;
                let root_node = tree.root_node();

                let query = Query::new(
                    &lang,
                    r#"
                        (class_declaration
                            (modifier)* @mod
                            name: (identifier) @class
                        )
                    "#,
                )
                .expect("Invalid query");
                let mut cursor = QueryCursor::new();
                let class_nodes = cursor
                    .captures(&query, root_node, source_code.as_bytes())
                    .filter_map_deref(|x| {
                        let mut class: Option<_> = None;
                        let mut mods = vec![];
                        x.0.captures.iter().for_each(|y| {
                            if y.node.kind() == "identifier" {
                                class = Some(y.node.parent().unwrap());
                            } else if y.node.kind() == "modifier" {
                                mods.push(
                                    y.node
                                        .utf8_text(source_code.as_bytes())
                                        .expect("Error decoding text"),
                                );
                            }
                        });
                        if let Some(id) = class {
                            Some((id, mods))
                        } else {
                            None
                        }
                    })
                    .collect::<Vec<_>>();

                for (node, mods) in class_nodes {
                    let name = node
                        .child_by_field_name("name".as_bytes())?
                        .utf8_text(source_code.as_bytes())
                        .expect("Error decoding text")
                        .to_string();
                    println!("Class: {}", name);
                    println!("\t Modifiers: {:?}", mods);

                    let query = Query::new(
                        &lang,
                        r#"
                            (class_declaration
                                body: (declaration_list
                                    (field_declaration
                                        (modifier)* @mod
                                        (variable_declaration
                                            (variable_declarator) @var
                                        ) @parent
                                    )
                                )
                            )
                        "#,
                    )
                    .expect("Invalid query");
                    let mut cursor = QueryCursor::new();
                    let fields = cursor
                        .captures(&query, node, source_code.as_bytes())
                        .filter_map_deref(|x| {
                            let mut ty: Option<_> = None;
                            let mut name: Option<_> = None;
                            let mut default: Option<_> = None;
                            let mut mods = vec![];
                            x.0.captures.iter().for_each(|y| {
                                if let Some(val) = y.node.child_by_field_name("type".as_bytes()) {
                                    ty = Some(val);
                                } else if let Some(val) =
                                    y.node.child_by_field_name("name".as_bytes())
                                {
                                    name = Some(
                                        val.utf8_text(source_code.as_bytes())
                                            .expect("Error decoding text"),
                                    );
                                    let mut cursor = y.node.walk();
                                    default = y
                                        .node
                                        .children(&mut cursor)
                                        .filter_map(|z| {
                                            if z.kind().ends_with("literal") {
                                                Some(
                                                    z.utf8_text(source_code.as_bytes())
                                                        .expect("Error decoding text"),
                                                )
                                            } else {
                                                None
                                            }
                                        })
                                        .collect::<Vec<_>>()
                                        .first()
                                        .cloned();
                                } else if y.node.kind() == "modifier" {
                                    mods.push(
                                        y.node
                                            .utf8_text(source_code.as_bytes())
                                            .expect("Error decoding text"),
                                    );
                                }
                            });
                            if ty.is_some() && name.is_some() {
                                Some((name.unwrap(), ty.unwrap(), default, mods))
                            } else {
                                None
                            }
                        })
                        .collect::<Vec<_>>();

                    let mut const_fields = vec![];
                    let normal_fields = fields
                        .iter()
                        .filter_map(|(name, ty, default, mods)| {
                            if mods.contains(&"const") {
                                const_fields.push((*name, *ty, *default, mods.clone()));
                            } else {
                                let mods = mods
                                    .iter()
                                    .filter_map(|x| state.decode_modifier(x))
                                    .collect::<Vec<_>>();
                                let name = format_ident!("{}", name);
                                let ty = state.decode_type(ty, &source_code);
                                if let Some(ty) = ty {
                                    return Some(quote! {
                                      #(#mods) * #name: #ty
                                    });
                                }
                            }
                            None
                        })
                        .collect::<Vec<_>>();

                    let name_ident = format_ident!("{}", name);
                    let mods = mods
                        .iter()
                        .filter_map(|x| state.decode_modifier(x))
                        .collect::<Vec<_>>();
                    let mut code = quote! {};
                    if normal_fields.len() > 0 {
                        code.extend(quote! {
                          #(#mods) * struct #name_ident {
                            #(#normal_fields),*
                          }
                        });
                    } else {
                        code.extend(quote! {
                          #(#mods) * struct #name_ident {}
                        });
                    }

                    let static_fields = const_fields
                        .iter()
                        .filter_map(|(name, ty, default, mods)| {
                            let name = format_ident!("{}", name);
                            let ty = state.decode_type(ty, &source_code);
                            let mods = mods
                                .iter()
                                .filter_map(|x| state.decode_modifier(x))
                                .collect::<Vec<_>>();
                            if ty.is_some() {
                                let default = state.decode_expression(
                                    default.unwrap(),
                                    &ty.clone().unwrap().to_string(),
                                );
                                Some(quote! {
                                    #(#mods) * #name: #ty = #default;
                                })
                            } else {
                                None
                            }
                        })
                        .collect::<Vec<_>>();

                    code.extend(quote! {
                        impl #name_ident {
                          #(#static_fields)*
                        }
                    });

                    // let query = Query::new(
                    //     &lang,
                    //     r#"
                    //         (class_declaration
                    //             body: (declaration_list
                    //                 (property_declaration) @method
                    //             )
                    //         )
                    //   "#,
                    // )
                    // .expect("Invalid Query");

                    // let mut cursor = QueryCursor::new();
                    // let methods = cursor
                    //     .captures(&query, node, source_code.as_bytes())
                    //     .map_deref(|x| x.0.captures)
                    //     .flatten()
                    //     .map(|x| x.node)
                    //     .collect::<Vec<_>>();

                    // println!("\tMethods: {}", methods.len());

                    println!("{}", code.to_string());
                }

                return Some(());
            }
            None
        })
        .collect::<Vec<_>>();

    println!("\nFiles: {}", files.len());

    Ok(())
}

struct State {
    _declared_identifiers: Vec<String>,
    unknown_identifiers: Vec<String>,
    types: HashMap<&'static str, Ident>,
}

impl State {
    fn new() -> Self {
        let mut types = HashMap::new();
        types.insert("int", format_ident!("{}", "i32"));
        types.insert("float", format_ident!("{}", "f32"));
        types.insert("bool", format_ident!("{}", "bool"));

        Self {
            _declared_identifiers: vec![],
            unknown_identifiers: vec![],
            types,
        }
    }

    fn decode_type(&mut self, node: &Node, source_code: &str) -> Option<TokenStream> {
        match node.kind() {
            "identifier" => {
                let text = node
                    .utf8_text(source_code.as_bytes())
                    .expect("Error decoding text")
                    .to_owned();
                self.unknown_identifiers.push(text.clone());
                let id = format_ident!("{}", text);
                Some(quote! { #id })
            }
            "predefined_type" => {
                if let Some(type_name) = self.types.get(
                    node.utf8_text(source_code.as_bytes())
                        .expect("Error decoding text"),
                ) {
                    let id = format_ident!("{}", type_name);
                    Some(quote! { #id })
                } else {
                    None
                }
            }
            "array_type" => {
                let mut array_type =
                    self.decode_type(&node.child_by_field_name("type".as_bytes())?, source_code)?;
                let rank = node
                    .child_by_field_name("rank".as_bytes())?
                    .utf8_text(source_code.as_bytes())
                    .expect("Error decoding text")
                    .chars()
                    .filter(|x| x == &'[')
                    .count();
                for _ in 0..rank {
                    array_type = quote! { [#array_type] };
                }
                Some(quote! { Box<#array_type> })
            }
            _ => None,
        }
    }

    fn decode_modifier(&self, modifier: &str) -> Option<Ident> {
        match modifier {
            "const" => Some(format_ident!("const")),
            "public" => Some(format_ident!("pub")),
            _ => None,
        }
    }

    fn decode_expression(&self, expression: &str, ty: &str) -> Option<TokenStream> {
        match ty {
            "i32" => {
                let expr = expression.parse::<i32>().unwrap();
                Some(quote! { #expr })
            }
            "f32" => {
                let expr = expression.replace("f", "").parse::<f32>().unwrap();
                Some(quote! { #expr })
            }
            _ => None,
        }
    }
}
