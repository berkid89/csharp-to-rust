//! Transpile a small subset of C# into equivalent Rust.
//!
//! [`transpile`](transpile) converts one public static method into a Rust function.
//! [`transpile_method`](transpile_method) also returns the method signature and
//! attribute names. The class that contains the method is a C# wrapper and is not emitted.

mod check;
mod emit;

/// A C# primitive type mapped to Rust.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrimitiveType {
    /// C# `int`.
    I32,
    /// C# `float`.
    F32,
    /// C# `double`.
    F64,
    /// C# `bool`.
    Bool,
}

/// A parameter of the generated Rust function.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Parameter {
    /// The Rust parameter name.
    pub name: String,
    /// The mapped primitive type.
    pub primitive_type: PrimitiveType,
}

/// One transpiled C# method.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TranspiledMethod {
    /// The method name as written in C#.
    pub csharp_name: String,
    /// The generated Rust function name.
    pub rust_name: String,
    /// Parameters in source order, using their Rust names and types.
    pub parameters: Vec<Parameter>,
    /// The mapped return type.
    pub return_type: PrimitiveType,
    /// Simple attribute names, in source order. These are labels only.
    pub attributes: Vec<String>,
    /// The generated Rust function, including a trailing newline.
    pub rust_source: String,
}

/// Transpile C# source into Rust source.
///
/// On success, the string is a single `pub fn` followed by a newline.
/// On failure, the string describes the syntax error or unsupported construct.
pub fn transpile(source: &str) -> Result<String, String> {
    transpile_method(source).map(|method| method.rust_source)
}

/// Transpile C# source and return the signature, attribute names, and Rust source.
pub fn transpile_method(source: &str) -> Result<TranspiledMethod, String> {
    let method = check::check(source)?;
    let rust_source = emit::emit(&method);
    Ok(TranspiledMethod {
        csharp_name: method.csharp_name,
        rust_name: method.rust_name,
        parameters: method.parameters,
        return_type: method.return_type,
        attributes: method.attributes,
        rust_source,
    })
}
