//! Read-only intermediate representation of a type-checked C# method.
//!
//! [`crate::analyze_method`] is the only way to obtain these values. Callers
//! can inspect them and pass them to [`crate::emit_rust`], but cannot build or
//! modify a method that skipped semantic checking.
//!
//! Expression names and `let` bindings keep the resolved Rust names. Parameters
//! also keep the original C# names.
//!
//! This is an early public API and may change in future 0.x releases.
//! Downstream tools can walk the typed tree without generating Rust first.

use super::PrimitiveType;

/// A validated C# method.
///
/// Produced only by [`crate::analyze_method`]. Fields are private so a caller
/// cannot assemble a method that bypassed checking and then emit it.
#[derive(Debug)]
pub struct Method {
    pub(crate) csharp_name: String,
    pub(crate) rust_name: String,
    pub(crate) parameters: Vec<Parameter>,
    pub(crate) return_type: PrimitiveType,
    pub(crate) attributes: Vec<String>,
    pub(crate) body: Vec<Statement>,
}

impl Method {
    /// The method name as written in C#.
    pub fn csharp_name(&self) -> &str {
        &self.csharp_name
    }

    /// The Rust function name chosen for this method.
    pub fn rust_name(&self) -> &str {
        &self.rust_name
    }

    /// Parameters in source order.
    pub fn parameters(&self) -> &[Parameter] {
        &self.parameters
    }

    /// The mapped return type.
    pub fn return_type(&self) -> PrimitiveType {
        self.return_type
    }

    /// Simple attribute names, in source order.
    ///
    /// These are labels only. They are not interpreted by this crate.
    pub fn attributes(&self) -> &[String] {
        &self.attributes
    }

    /// The type-checked method body, in source order.
    pub fn body(&self) -> &[Statement] {
        &self.body
    }
}

/// A parameter of a checked method.
///
/// `csharp_name` is the name written in C#. `rust_name` is the distinct name
/// used in generated Rust.
#[derive(Debug)]
pub struct Parameter {
    pub(crate) csharp_name: String,
    pub(crate) rust_name: String,
    pub(crate) primitive_type: PrimitiveType,
}

impl Parameter {
    /// The parameter name as written in C#.
    pub fn csharp_name(&self) -> &str {
        &self.csharp_name
    }

    /// The Rust parameter name chosen for this parameter.
    pub fn rust_name(&self) -> &str {
        &self.rust_name
    }

    /// The mapped primitive type.
    pub fn primitive_type(&self) -> PrimitiveType {
        self.primitive_type
    }
}

/// A type-checked statement.
#[derive(Debug)]
pub enum Statement {
    /// `return` with a type-checked expression.
    Return(Expr),
    /// Initialized locals. Each name is the resolved Rust name.
    Let(Vec<(String, Expr)>),
    /// A nested block.
    Block(Vec<Statement>),
    /// `if`, with an optional `else`.
    If {
        /// The condition. Its type is `bool`.
        condition: Expr,
        /// The branch taken when the condition is true.
        consequence: Box<Statement>,
        /// The branch taken when the condition is false.
        alternative: Option<Box<Statement>>,
    },
}

impl Statement {
    pub(crate) fn always_returns(&self) -> bool {
        match self {
            Self::Return(_) => true,
            Self::Block(statements) => statements.iter().any(Self::always_returns),
            Self::If {
                consequence,
                alternative,
                ..
            } => {
                consequence.always_returns()
                    && alternative.as_deref().is_some_and(Self::always_returns)
            }
            Self::Let(_) => false,
        }
    }
}

/// A type-checked expression and the type it was checked as.
#[derive(Debug)]
pub struct Expr {
    pub(crate) kind: ExprKind,
    pub(crate) ty: PrimitiveType,
}

impl Expr {
    /// The expression's structure.
    pub fn kind(&self) -> &ExprKind {
        &self.kind
    }

    /// The type this expression was checked as.
    pub fn ty(&self) -> PrimitiveType {
        self.ty
    }

    pub(crate) fn integer(value: i32) -> Self {
        Self {
            kind: ExprKind::Integer(value),
            ty: PrimitiveType::I32,
        }
    }

    pub(crate) fn constant_integer(&self) -> Option<i32> {
        match &self.kind {
            ExprKind::Integer(value) => Some(*value),
            ExprKind::Group(inner) => inner.constant_integer(),
            _ => None,
        }
    }
}

/// The structure of a type-checked expression.
///
/// Names stored here are resolved Rust names, not the original C# spellings.
#[derive(Debug)]
pub enum ExprKind {
    /// A parameter or local.
    Name(String),
    /// An `int` literal.
    Integer(i32),
    /// The numeric text of a `float` or `double` literal, without its suffix.
    Real(String),
    /// `true` or `false`.
    Bool(bool),
    /// Parentheses that were present in the C# source.
    Group(Box<Expr>),
    /// A unary operator and its operand.
    Unary(UnaryOp, Box<Expr>),
    /// A binary operator and its operands, in source order.
    Binary(BinaryOp, Box<Expr>, Box<Expr>),
}

/// A checked unary operator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOp {
    /// Arithmetic negation.
    Negate,
    /// Boolean negation.
    Not,
}

/// A checked binary operator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryOp {
    /// `+`.
    Add,
    /// `-`.
    Subtract,
    /// `*`.
    Multiply,
    /// `/`.
    Divide,
    /// `<`.
    Less,
    /// `<=`.
    LessEqual,
    /// `>`.
    Greater,
    /// `>=`.
    GreaterEqual,
    /// `==`.
    Equal,
    /// `!=`.
    NotEqual,
    /// `&&`.
    And,
    /// `||`.
    Or,
}
