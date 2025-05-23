#![allow(unused)]

use std::{collections::HashMap, env, fs, path::Path, process::Command};

use anyhow::{Context, Result, anyhow};
// use proc_macro2::{Ident, TokenStream};
// use quote::{format_ident, quote};
use rand::Rng;
use rand::distr::Alphanumeric;
use tree_sitter::{Language, Node, Parser, Query, QueryCursor, QueryMatch, StreamingIterator};

mod types;

use types::{Chunk, Class, Enum, Interface, Method, Modifier, Primitive, Struct, Type, Variable};

fn main() -> Result<()> {
    let args = env::args();
    if args.len() != 2 {
        println!("Usage: tsdsp <dll>");
        println!("`ilspycmd` dotnet tool required.");
        return Ok(());
    }
    let args = args.collect::<Vec<String>>();
    let dll_name = args[1].as_str();

    // let tmp_dir = env::temp_dir();
    // let id = rand::rng()
    //     .sample_iter(Alphanumeric)
    //     .map(|x| x as char)
    //     .take(16)
    //     .collect::<String>();
    // let work_dir = tmp_dir.join("tsdsp").join(id);
    // fs::create_dir_all(&work_dir)?;
    let work_dir = Path::new(
        "/private/var/folders/_q/yycj0b893pzccg6kxlnf3m640000gn/T/tsdsp/Rw6ttZ8oTMi0jw5w",
    );
    let work_dir = work_dir.canonicalize()?;

    Command::new("ilspycmd")
        .output()
        .context("Could not run `ilspycmd`")?;

    println!("Extracting {} to {:?}", dll_name, work_dir);

    // let res = Command::new("ilspycmd")
    //     .arg("-p")
    //     .arg("-o")
    //     .arg(&work_dir)
    //     .arg(dll_name)
    //     .output()
    //     .context("Could not run `ilspycmd`")?;

    // if !res.status.success() {
    //     Err(anyhow!("ilspycmd failed"))?;
    // }

    let mut parser = Parser::new();
    let lang: Language = tree_sitter_c_sharp::LANGUAGE.into();
    parser
        .set_language(&lang)
        .expect("Error loading C# grammar");

    let mut chunks = Vec::new();
    let mut dir = fs::read_dir(&work_dir)?;
    for (i, file) in dir.enumerate() {
        let path = file?.path();
        if !path.is_file() || path.extension().unwrap() != "cs" {
            continue;
        }
        chunks.push(parse_file(&mut parser, &lang, path)?);

        if i % 50 == 0 {
            println!("Processed {} files", i);
        }
    }

    let counts = chunks.iter().fold(
        (0, 0, 0, 0, 0),
        |(classes, enums, structs, interfaces, delegates), chunk| match chunk {
            Chunk::Class(_) => (classes + 1, enums, structs, interfaces, delegates),
            Chunk::Enum(_) => (classes, enums + 1, structs, interfaces, delegates),
            Chunk::Struct(_) => (classes, enums, structs + 1, interfaces, delegates),
            Chunk::Interface(_) => (classes, enums, structs, interfaces + 1, delegates),
            Chunk::Delegate(_) => (classes, enums, structs, interfaces, delegates + 1),
        },
    );

    println!(
        "Classes: {}, Enums: {}, Structs: {}, Interfaces: {}, Delegates: {}",
        counts.0, counts.1, counts.2, counts.3, counts.4
    );

    Ok(())
}

fn parse_file(parser: &mut Parser, lang: &Language, path: std::path::PathBuf) -> Result<Chunk> {
    let source = fs::read_to_string(path.clone())?;
    let tree = parser
        .parse(source.clone(), None)
        .expect("Could not parse C# file");
    let root = tree.root_node();

    let query_type = Query::new(
        lang,
        r#"
        [
            (class_declaration)
            (enum_declaration)
            (struct_declaration)
            (interface_declaration)
            (delegate_declaration)
        ] @type
        "#,
    )
    .expect("Could not create query");

    let mut cursor = QueryCursor::new();
    let chunk_type = cursor
        .matches(&query_type, root, source.as_bytes())
        .map_deref(|x| x.captures)
        .next()
        .expect(&format!("No chunk found in {:?}", path.clone()))
        .iter()
        .next()
        .expect("Empty capture")
        .node
        .kind();

    match chunk_type {
        "class_declaration" => extract_class(lang, root, &source),
        "enum_declaration" => extract_enum(lang, root, &source),
        "struct_declaration" => extract_struct(lang, root, &source),
        "interface_declaration" => extract_interface(lang, root, &source),
        "delegate_declaration" => extract_delegate(lang, root, &source),
        _ => Err(anyhow!("Unknown chunk type")),
    }
}

fn extract_delegate(lang: &Language, node: Node, source: &str) -> Result<Chunk> {
    let delegate_query = Query::new(
        lang,
        r#"
        (compilation_unit
            (using_directive
                [
                    (identifier)
                    (qualified_name)
                ] @directive
            )*
            (delegate_declaration
                (modifier)* @modifier
                type: (_) @type
                name: (identifier) @name
                parameters: (parameter_list
                    (parameter)* @parameter
                )
                body: (block)? @body
            )
        )
        "#,
    )
    .expect("Failed to create query");

    let captures = capture(&delegate_query, node, source);

    let name = captures
        .get("name")
        .expect("Invalid field declaration")
        .first()
        .expect("Empty field name")
        .utf8_text(source.as_bytes())
        .expect("Error decoding text")
        .to_string();

    let mods = captures
        .get("modifier")
        .expect("Invalid field declaration")
        .iter()
        .map(|x| Modifier::from(x.utf8_text(source.as_bytes()).expect("Error decoding text")))
        .collect::<Vec<_>>();

    let ty = decode_type(
        captures
            .get("type")
            .expect("Invalid field declaration")
            .first()
            .expect("Empty type"),
        source,
    );

    let params = captures
        .get("parameter")
        .expect("Invalid field declaration")
        .iter()
        .map(|x| {
            (
                x.child_by_field_name("name")
                    .expect("Invalid field name")
                    .utf8_text(source.as_bytes())
                    .expect("Error decoding text")
                    .to_string(),
                decode_type(
                    &x.child_by_field_name("type").expect("Invalid field type"),
                    source,
                ),
            )
        })
        .collect::<HashMap<_, _>>();

    let body = captures
        .get("body")
        .expect("Invalid field declaration")
        .first()
        .map(|x| {
            x.utf8_text(source.as_bytes())
                .expect("Error decoding text")
                .to_string()
        })
        .unwrap_or(String::new());

    Ok(Chunk::Delegate(Method::new(name, mods, ty, params, body)))
}

fn extract_interface(lang: &Language, node: Node, source: &str) -> Result<Chunk> {
    let interface_query = Query::new(
        lang,
        r#"
        (compilation_unit
            (using_directive
                [
                    (identifier)
                    (qualified_name)
                ] @directive
            )*
            (interface_declaration
                (modifier)* @modifier
                (identifier) @name
                (base_list
                    (identifier) @base
                )?
                (declaration_list
                    [
                        (property_declaration)* @property
                        (method_declaration)* @method
                    ]*
                )?
            )
        )
        "#,
    )
    .expect("Error creating query");

    let captures = capture(&interface_query, node, source);

    let name = captures
        .get("name")
        .expect("Invalid struct declaration")
        .first()
        .expect("Missing struct name")
        .utf8_text(source.as_bytes())
        .expect("Error decoding text")
        .to_string();

    let mods = captures
        .get("modifier")
        .expect("Invalid struct declaration")
        .iter()
        .map(|m| Modifier::from(m.utf8_text(source.as_bytes()).expect("Error decoding text")))
        .collect::<Vec<_>>();

    let base = captures
        .get("base")
        .expect("Invalid struct declaration")
        .iter()
        .map(
            |x| match x.utf8_text(source.as_bytes()).expect("Error decoding text") {
                "Object" => Type::Primitive(Primitive::Object),
                s => Type::Object(s.to_string(), None),
            },
        )
        .collect::<Vec<_>>();

    let mut res = Interface::new(name, mods, base);

    captures
        .get("method")
        .expect("Invalid class declaration")
        .iter()
        .cloned()
        .map(|x| extract_method(&lang, x, &source).expect("Error decoding method"))
        .for_each(|f| res.add_method(f));

    Ok(Chunk::Interface(res))
}

fn extract_struct(lang: &Language, node: Node, source: &str) -> Result<Chunk> {
    let struct_query = Query::new(
        lang,
        r#"
        (compilation_unit
            (using_directive
                [
                    (identifier)
                    (qualified_name)
                ] @directive
            )*
            (struct_declaration
                (modifier)* @modifier
                (identifier) @name
                (base_list
                    (identifier) @base
                )?
                (declaration_list
                    [
                        (field_declaration)* @field
                        (method_declaration)* @method
                    ]*
                )?
            )
        )
        "#,
    )
    .expect("Error creating query");

    let captures = capture(&struct_query, node, source);

    let name = captures
        .get("name")
        .expect("Invalid struct declaration")
        .first()
        .expect("Missing struct name")
        .utf8_text(source.as_bytes())
        .expect("Error decoding text")
        .to_string();

    let mods = captures
        .get("modifier")
        .expect("Invalid struct declaration")
        .iter()
        .map(|m| Modifier::from(m.utf8_text(source.as_bytes()).expect("Error decoding text")))
        .collect::<Vec<_>>();

    let base = captures
        .get("base")
        .expect("Invalid struct declaration")
        .iter()
        .map(
            |x| match x.utf8_text(source.as_bytes()).expect("Error decoding text") {
                "Object" => Type::Primitive(Primitive::Object),
                s => Type::Object(s.to_string(), None),
            },
        )
        .collect::<Vec<_>>();

    let mut res = Struct::new(name, mods, base);

    captures
        .get("field")
        .expect("Invalid class declaration")
        .iter()
        .cloned()
        .map(|x| extract_field(&lang, x, &source).expect("Error decoding field"))
        .for_each(|f| res.add_variable(f));

    captures
        .get("method")
        .expect("Invalid class declaration")
        .iter()
        .cloned()
        .map(|x| extract_method(&lang, x, &source).expect("Error decoding method"))
        .for_each(|f| res.add_method(f));

    Ok(Chunk::Struct(res))
}

fn extract_enum(lang: &Language, node: Node, source: &str) -> Result<Chunk> {
    let enum_query = Query::new(
        lang,
        r#"
        (compilation_unit
            (using_directive
                [
                    (identifier)
                    (qualified_name)
                ] @directive
            )*
            (enum_declaration
            	  (modifier)* @modifier
                (identifier) @name
                (base_list
                    (identifier) @base
                )?
                (enum_member_declaration_list
                	  (enum_member_declaration)* @member
                )
            )
        )
        "#,
    )
    .expect("Failed to create query");

    let captures = capture(&enum_query, node, &source);

    let name = captures
        .get("name")
        .expect("Invalid class declaration")
        .first()
        .expect("No class name")
        .utf8_text(source.as_bytes())
        .expect("Error decoding text")
        .to_string();

    let mods = captures
        .get("modifier")
        .expect("Invalid class declaration")
        .iter()
        .map(|x| Modifier::from(x.utf8_text(source.as_bytes()).expect("Error decoding text")))
        .collect::<Vec<_>>();

    let base = captures
        .get("base")
        .expect("Invalid class declaration")
        .iter()
        .map(
            |x| match x.utf8_text(source.as_bytes()).expect("Error decoding text") {
                "Object" => Type::Primitive(Primitive::Object),
                s => Type::Object(s.to_string(), None),
            },
        )
        .collect::<Vec<_>>();

    let mut res = Enum::new(name, mods, base);

    captures
        .get("member")
        .expect("Invalid enum declaration")
        .iter()
        .cloned()
        .map(|x| {
            (
                x.child_by_field_name("name")
                    .unwrap()
                    .utf8_text(source.as_bytes())
                    .expect("Error decoding text")
                    .to_string(),
                x.child_by_field_name("value").map(|y| {
                    y.utf8_text(source.as_bytes())
                        .expect("Error decoding text")
                        .parse::<i32>()
                        .expect(&format!("Enum not as i32: {}", source))
                }),
            )
        })
        .for_each(|(k, v)| res.add_value(k, v));

    Ok(Chunk::Enum(res))
}

fn extract_class(lang: &Language, node: Node, source: &str) -> Result<Chunk> {
    let class_query = Query::new(
        lang,
        r#"
        (compilation_unit
            (using_directive
                [
                    (identifier)
                    (qualified_name)
                ] @directive
            )*
            (class_declaration
                (modifier)* @modifier
                name: (identifier) @name
                (base_list
                    (identifier) @base
                )?
                (declaration_list
                    [
                        (field_declaration)* @field
                        (property_declaration)* @property
                        (constructor_declaration)* @constructor
                        (destructor_declaration)* @destructor
                        (method_declaration)* @method
                    ]*
                )?
            )
        )
        "#,
    )
    .expect("Invalid query");

    let captures = capture(&class_query, node, &source);

    let name = captures
        .get("name")
        .expect("Invalid class declaration")
        .first()
        .expect("No class name")
        .utf8_text(source.as_bytes())
        .expect("Error decoding text")
        .to_string();

    let mods = captures
        .get("modifier")
        .expect("Invalid class declaration")
        .iter()
        .map(|x| Modifier::from(x.utf8_text(source.as_bytes()).expect("Error decoding text")))
        .collect::<Vec<_>>();

    let base = captures
        .get("base")
        .expect("Invalid class declaration")
        .iter()
        .map(
            |x| match x.utf8_text(source.as_bytes()).expect("Error decoding text") {
                "Object" => Type::Primitive(Primitive::Object),
                s => Type::Object(s.to_string(), None),
            },
        )
        .collect::<Vec<_>>();

    let mut cls = Class::new(name, mods, base);

    captures
        .get("field")
        .expect("Invalid class declaration")
        .iter()
        .cloned()
        .map(|x| extract_field(&lang, x, &source).expect("Error decoding field"))
        .for_each(|f| cls.add_variable(f));

    captures
        .get("method")
        .expect("Invalid class declaration")
        .iter()
        .cloned()
        .map(|x| extract_method(&lang, x, &source).expect("Error decoding method"))
        .for_each(|f| cls.add_method(f));

    Ok(Chunk::Class(cls))
}

fn capture<'a>(query: &Query, node: Node<'a>, source: &str) -> HashMap<String, Vec<Node<'a>>> {
    let mut cursor = QueryCursor::new();
    let res = cursor
        .matches(query, node, source.as_bytes())
        .map_deref(|y| y.captures)
        .next()
        .expect(&format!(
            "Invalid query for node: {}",
            node.utf8_text(source.as_bytes())
                .expect("Error decoding text")
        ));

    query
        .capture_names()
        .iter()
        .filter_map(|&x| {
            if let Some(index) = query.capture_index_for_name(x) {
                Some((x, index))
            } else {
                None
            }
        })
        .map(|(x, i)| {
            (
                x.to_string(),
                res.iter()
                    .filter_map(|y| if y.index == i { Some(y.node) } else { None })
                    .collect::<Vec<_>>(),
            )
        })
        .collect()
}

fn extract_field(lang: &Language, node: Node, source: &str) -> Result<Variable> {
    let field_query = Query::new(
        lang,
        r#"
        (field_declaration
            (modifier)* @modifier
            (variable_declaration
                type: (_) @type
                (variable_declarator
                    name: (identifier) @name
                    [
                        (integer_literal)
                        (real_literal)
                        (string_literal)
                        (boolean_literal)
                    ]? @value
                )
            )
        )
        "#,
    )
    .expect("Invalid query");

    let captures = capture(&field_query, node, source);

    let name = captures
        .get("name")
        .expect("Invalid field declaration")
        .first()
        .expect("Empty field name")
        .utf8_text(source.as_bytes())
        .expect("Error decoding text")
        .to_string();

    let mods = captures
        .get("modifier")
        .expect("Invalid field declaration")
        .iter()
        .map(|x| Modifier::from(x.utf8_text(source.as_bytes()).expect("Error decoding text")))
        .collect::<Vec<_>>();

    let ty = decode_type(
        captures
            .get("type")
            .expect("Invalid field declaration")
            .first()
            .expect("Empty type"),
        source,
    );

    let value = captures
        .get("value")
        .expect("Invalid field declaration")
        .first()
        .map(|x| {
            x.utf8_text(source.as_bytes())
                .expect("Error decoding text")
                .to_string()
        });

    Ok(Variable::new(name, mods, ty, value))
}

fn decode_type(node: &Node, source: &str) -> Type {
    match node.kind() {
        "identifier" => Type::Object(
            node.utf8_text(source.as_bytes())
                .expect("Error decoding text")
                .to_string(),
            None,
        ),
        "predefined_type" => Type::Primitive(Primitive::from(
            node.utf8_text(source.as_bytes())
                .expect("Error decoding text"),
        )),
        "array_type" => {
            let sub_type = decode_type(&node.child_by_field_name("type").unwrap(), source);
            match sub_type {
                Type::Array(ty, rank) => Type::Array(ty, rank + 1),
                _ => Type::Array(Box::new(sub_type), 1),
            }
        }
        "generic_name" => {
            let children = node.children(&mut node.walk()).collect::<Vec<_>>();
            let name = children
                .iter()
                .filter(|&x| x.grammar_name() == "identifier")
                .next()
                .expect("Invalid generic_name node")
                .utf8_text(source.as_bytes())
                .expect("Error decoding text")
                .to_string();
            let sub_type = decode_type(
                &children
                    .iter()
                    .filter(|x| x.grammar_name() == "type_argument_list")
                    .next()
                    .expect("Invalid generic_name node")
                    .child(1)
                    .expect("Invalid type argument list"),
                source,
            );
            Type::Object(name, Some(Box::new(sub_type)))
        }
        "qualified_name" => {
            let name = node
                .utf8_text(source.as_bytes())
                .expect("Error decoding text")
                .to_string();
            Type::Object(name, None)
        }
        "ref_type" => {
            let sub_type = decode_type(
                &node.child_by_field_name("type").expect("Invalid ref type"),
                source,
            );
            Type::Reference(Box::new(sub_type))
        }
        "alias_qualified_name" => {
            let name = node
                .utf8_text(source.as_bytes())
                .expect("Error decoding text")
                .to_string();
            Type::Object(name, None)
        }
        _ => panic!(
            "Invalid type: {}",
            node.utf8_text(source.as_bytes())
                .expect("Error decoding text")
        ),
    }
}

fn extract_method(lang: &Language, node: Node, source: &str) -> Result<Method> {
    let method_query = Query::new(
        lang,
        r#"
        (method_declaration
            (modifier)* @modifier
            returns: (_) @type
            name: (identifier) @name
            parameters: (parameter_list
                (parameter)* @parameter
            )
            body: (block)? @body
        )
        "#,
    )
    .expect("Invalid query");

    let captures = capture(&method_query, node, source);

    let name = captures
        .get("name")
        .expect("Invalid field declaration")
        .first()
        .expect("Empty field name")
        .utf8_text(source.as_bytes())
        .expect("Error decoding text")
        .to_string();

    let mods = captures
        .get("modifier")
        .expect("Invalid field declaration")
        .iter()
        .map(|x| Modifier::from(x.utf8_text(source.as_bytes()).expect("Error decoding text")))
        .collect::<Vec<_>>();

    let ty = decode_type(
        captures
            .get("type")
            .expect("Invalid field declaration")
            .first()
            .expect("Empty type"),
        source,
    );

    let params = captures
        .get("parameter")
        .expect("Invalid field declaration")
        .iter()
        .map(|x| {
            (
                x.child_by_field_name("name")
                    .expect("Invalid field name")
                    .utf8_text(source.as_bytes())
                    .expect("Error decoding text")
                    .to_string(),
                decode_type(
                    &x.child_by_field_name("type").expect("Invalid field type"),
                    source,
                ),
            )
        })
        .collect::<HashMap<_, _>>();

    let body = captures
        .get("body")
        .expect("Invalid field declaration")
        .first()
        .map(|x| {
            x.utf8_text(source.as_bytes())
                .expect("Error decoding text")
                .to_string()
        })
        .unwrap_or(String::new());

    Ok(Method::new(name, mods, ty, params, body))
}
