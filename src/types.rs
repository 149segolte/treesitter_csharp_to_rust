use std::collections::HashMap;

#[derive(Debug, Clone, Default)]
pub enum Primitive {
    #[default]
    Void,
    Bool,
    Short,
    Ushort,
    Int,
    Uint,
    Long,
    Ulong,
    Float,
    Double,
    String,
    Byte,
    Sbyte,
    Char,
    Object,
}

impl From<&str> for Primitive {
    fn from(s: &str) -> Self {
        match s {
            "void" => Primitive::Void,
            "bool" => Primitive::Bool,
            "short" => Primitive::Short,
            "ushort" => Primitive::Ushort,
            "int" => Primitive::Int,
            "uint" => Primitive::Uint,
            "long" => Primitive::Long,
            "ulong" => Primitive::Ulong,
            "float" => Primitive::Float,
            "double" => Primitive::Double,
            "string" => Primitive::String,
            "byte" => Primitive::Byte,
            "sbyte" => Primitive::Sbyte,
            "char" => Primitive::Char,
            "object" => Primitive::Object,
            _ => panic!("Invalid primitive {}", s),
        }
    }
}

#[derive(Debug, Clone)]
pub enum Type {
    Primitive(Primitive),
    Array(Box<Type>, u8),
    Tuple(Vec<Type>),
    Reference(Box<Type>),
    Object(String, Option<Box<Type>>),
}

impl Default for Type {
    fn default() -> Self {
        Type::Primitive(Primitive::Void)
    }
}

#[derive(Debug, Clone, Default)]
pub enum Modifier {
    #[default]
    Public,
    Private,
    Protected,
    Static,
    Const,
    Override,
    Final,
    Abstract,
    Internal,
    Sealed,
    Virtual,
    Readonly,
    New,
    Unsafe,
    Extern,
}

impl From<&str> for Modifier {
    fn from(s: &str) -> Self {
        match s {
            "public" => Modifier::Public,
            "private" => Modifier::Private,
            "protected" => Modifier::Protected,
            "static" => Modifier::Static,
            "const" => Modifier::Const,
            "override" => Modifier::Override,
            "final" => Modifier::Final,
            "abstract" => Modifier::Abstract,
            "internal" => Modifier::Internal,
            "sealed" => Modifier::Sealed,
            "virtual" => Modifier::Virtual,
            "readonly" => Modifier::Readonly,
            "new" => Modifier::New,
            "unsafe" => Modifier::Unsafe,
            "extern" => Modifier::Extern,
            _ => panic!("Invalid modifier {}", s),
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct Variable {
    name: String,
    modifiers: Vec<Modifier>,
    type_: Type,
    value: Option<String>,
}

impl Variable {
    pub fn new(name: String, modifiers: Vec<Modifier>, type_: Type, value: Option<String>) -> Self {
        Variable {
            name,
            modifiers,
            type_,
            value,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct Method {
    name: String,
    modifiers: Vec<Modifier>,
    return_type: Type,
    parameters: HashMap<String, Type>,
    body: String,
}

impl Method {
    pub fn new(
        name: String,
        modifiers: Vec<Modifier>,
        return_type: Type,
        parameters: HashMap<String, Type>,
        body: String,
    ) -> Self {
        Method {
            name,
            modifiers,
            return_type,
            parameters,
            body,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct Class {
    name: String,
    modifiers: Vec<Modifier>,
    base_class: Vec<Type>,
    variables: Vec<Variable>,
    methods: Vec<Method>,
}

impl Class {
    pub fn new(name: String, modifiers: Vec<Modifier>, base_class: Vec<Type>) -> Self {
        Class {
            name,
            modifiers,
            base_class,
            ..Default::default()
        }
    }

    pub fn add_variable(&mut self, variable: Variable) {
        self.variables.push(variable);
    }

    pub fn add_method(&mut self, method: Method) {
        self.methods.push(method);
    }
}

#[derive(Debug, Clone, Default)]
pub struct Enum {
    name: String,
    modifiers: Vec<Modifier>,
    base: Vec<Type>,
    values: HashMap<String, Option<i32>>,
}

impl Enum {
    pub fn new(name: String, modifiers: Vec<Modifier>, base: Vec<Type>) -> Self {
        Self {
            name,
            modifiers,
            base,
            ..Default::default()
        }
    }

    pub fn add_value(&mut self, name: String, value: Option<i32>) {
        self.values.insert(name, value);
    }
}

#[derive(Debug, Clone, Default)]
pub struct Struct {
    name: String,
    modifiers: Vec<Modifier>,
    base: Vec<Type>,
    variables: Vec<Variable>,
    methods: Vec<Method>,
}

impl Struct {
    pub fn new(name: String, modifiers: Vec<Modifier>, base: Vec<Type>) -> Self {
        Self {
            name,
            modifiers,
            base,
            ..Default::default()
        }
    }

    pub fn add_variable(&mut self, variable: Variable) {
        self.variables.push(variable);
    }

    pub fn add_method(&mut self, method: Method) {
        self.methods.push(method);
    }
}

#[derive(Debug, Clone, Default)]
pub struct Interface {
    name: String,
    modifiers: Vec<Modifier>,
    base: Vec<Type>,
    methods: Vec<Method>,
}

impl Interface {
    pub fn new(name: String, modifiers: Vec<Modifier>, base: Vec<Type>) -> Self {
        Self {
            name,
            modifiers,
            base,
            ..Default::default()
        }
    }

    pub fn add_method(&mut self, method: Method) {
        self.methods.push(method);
    }
}

#[derive(Debug, Clone)]
pub enum Chunk {
    Class(Class),
    Enum(Enum),
    Struct(Struct),
    Interface(Interface),
    Delegate(Method),
}
