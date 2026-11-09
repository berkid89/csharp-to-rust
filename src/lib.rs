//! Transpile a small subset of C# into equivalent Rust.
//!
//! Add this crate from another project:
//!
//! ```bash
//! cargo add csharp-to-rust
//! ```
//!
//! Then call [`transpile`]:
//!
//! ```
//! use csharp_to_rust::transpile;
//!
//! fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let csharp = r#"
//!         public static class Calculator {
//!             public static int Add(int a, int b) {
//!                 return a + b;
//!             }
//!         }
//!     "#;
//!
//!     let rust = transpile(csharp)?;
//!     println!("{rust}");
//! #    assert_eq!(
//! #        rust,
//! #        "pub fn add(a: i32, b: i32) -> i32 {\n    i32::wrapping_add(a, b)\n}\n"
//! #    );
//!     Ok(())
//! }
//! ```
//!
//! That prints one `pub fn`. The class is a C# wrapper and is not emitted.
//! [`transpile_method`] also returns the method signature and attribute names.
//!
//! The public API is [`transpile`], [`transpile_method`], [`Error`],
//! [`TranspiledMethod`], [`Parameter`], and [`PrimitiveType`].

#![deny(missing_docs)]

mod check;
mod emit;

/// Why transpilation failed.
///
/// Display this value to show the syntax error or unsupported construct.
/// The wording is for people and can change between releases.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Error {
    message: String,
}

impl Error {
    /// The diagnostic text.
    pub fn message(&self) -> &str {
        &self.message
    }
}

impl From<String> for Error {
    fn from(message: String) -> Self {
        Self { message }
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for Error {}

/// A C# primitive type mapped to Rust.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrimitiveType {
    /// C# `int`, emitted as `i32`.
    I32,
    /// C# `float`, emitted as `f32`.
    F32,
    /// C# `double`, emitted as `f64`.
    F64,
    /// C# `bool`, emitted as `bool`.
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
///
/// # Errors
///
/// Returns [`Error`] when the source is invalid or outside the supported subset.
///
/// # Examples
///
/// ```
/// use csharp_to_rust::transpile;
///
/// let csharp = "public class Box { public static int Id(int value) { return value; } }";
/// let rust = transpile(csharp)?;
/// assert!(rust.contains("pub fn id(value: i32)"));
/// # Ok::<(), csharp_to_rust::Error>(())
/// ```
pub fn transpile(source: &str) -> Result<String, Error> {
    transpile_method(source).map(|method| method.rust_source)
}

/// Transpile C# source and return the signature, attribute names, and Rust source.
///
/// # Errors
///
/// Returns [`Error`] in the same cases as [`transpile`].
///
/// # Examples
///
/// ```
/// use csharp_to_rust::transpile_method;
///
/// let csharp = "public class Box { public static int Id(int value) { return value; } }";
/// let method = transpile_method(csharp)?;
/// assert_eq!(method.csharp_name, "Id");
/// assert_eq!(method.rust_name, "id");
/// assert!(method.rust_source.starts_with("pub fn id(value: i32)"));
/// # Ok::<(), csharp_to_rust::Error>(())
/// ```
pub fn transpile_method(source: &str) -> Result<TranspiledMethod, Error> {
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
