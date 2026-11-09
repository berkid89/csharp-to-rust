use std::collections::HashSet;

use super::{Parameter, PrimitiveType};
use tree_sitter::{Node, Parser};

impl PrimitiveType {
    fn csharp(self) -> &'static str {
        match self {
            Self::I32 => "int",
            Self::F32 => "float",
            Self::F64 => "double",
            Self::Bool => "bool",
        }
    }

    fn is_numeric(self) -> bool {
        self != Self::Bool
    }
}

const RUST_KEYWORDS: &[&str] = &[
    "abstract", "as", "async", "await", "become", "box", "break", "const", "continue", "crate",
    "do", "dyn", "else", "enum", "extern", "false", "final", "fn", "for", "gen", "if", "impl",
    "in", "let", "loop", "macro", "match", "mod", "move", "mut", "override", "priv", "pub", "ref",
    "return", "self", "Self", "static", "struct", "super", "trait", "true", "try", "type",
    "typeof", "unsafe", "unsized", "use", "virtual", "where", "while", "yield",
];

pub(super) fn check(source: &str) -> Result<CheckedMethod, String> {
    let mut parser = Parser::new();
    parser
        .set_language(&tree_sitter_c_sharp::LANGUAGE.into())
        .map_err(|error| format!("failed to load the C# grammar: {error}"))?;

    let tree = parser
        .parse(source, None)
        .ok_or_else(|| "failed to parse C# source".to_string())?;
    let root = tree.root_node();
    if root.has_error() {
        return Err(syntax_error(root, source));
    }

    let class = single_class(root)?;
    let method = single_method(class)?;
    check_method(method, source)
}

pub(super) struct CheckedMethod {
    pub(super) csharp_name: String,
    pub(super) rust_name: String,
    pub(super) parameters: Vec<Parameter>,
    pub(super) return_type: PrimitiveType,
    pub(super) attributes: Vec<String>,
    pub(super) body: Vec<Statement>,
}

pub(super) enum Statement {
    Return(Expr),
    Let(Vec<(String, Expr)>),
    Block(Vec<Statement>),
    If {
        condition: Expr,
        consequence: Box<Statement>,
        alternative: Option<Box<Statement>>,
    },
}

impl Statement {
    fn always_returns(&self) -> bool {
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

pub(super) struct Expr {
    pub(super) kind: ExprKind,
    pub(super) ty: PrimitiveType,
}

pub(super) enum ExprKind {
    Name(String),
    Integer(i32),
    Real(String),
    Bool(bool),
    Group(Box<Expr>),
    Unary(UnaryOp, Box<Expr>),
    Binary(BinaryOp, Box<Expr>, Box<Expr>),
}

impl Expr {
    fn integer(value: i32) -> Self {
        Self {
            kind: ExprKind::Integer(value),
            ty: PrimitiveType::I32,
        }
    }

    fn constant_integer(&self) -> Option<i32> {
        match &self.kind {
            ExprKind::Integer(value) => Some(*value),
            ExprKind::Group(inner) => inner.constant_integer(),
            _ => None,
        }
    }
}

pub(super) enum UnaryOp {
    Negate,
    Not,
}

#[derive(Clone, Copy)]
pub(super) enum BinaryOp {
    Add,
    Subtract,
    Multiply,
    Divide,
    Less,
    LessEqual,
    Greater,
    GreaterEqual,
    Equal,
    NotEqual,
    And,
    Or,
}

struct Binding<'src> {
    csharp_name: &'src str,
    rust_name: String,
    ty: PrimitiveType,
}

#[derive(Default)]
struct Frame<'src> {
    bindings: Vec<Binding<'src>>,
    nested_declarations: HashSet<&'src str>,
}

struct Scope<'src> {
    frames: Vec<Frame<'src>>,
}

impl<'src> Scope<'src> {
    fn new() -> Self {
        Self {
            frames: vec![Frame::default()],
        }
    }

    fn lookup(&self, name: &str) -> Option<&Binding<'src>> {
        self.frames
            .iter()
            .rev()
            .flat_map(|frame| &frame.bindings)
            .find(|binding| binding.csharp_name == name)
    }

    fn declare(
        &mut self,
        csharp_name: &'src str,
        ty: PrimitiveType,
        role: &str,
        node: Node,
    ) -> Result<String, String> {
        let frame = self.frames.last().expect("scope has a frame");
        if self.lookup(csharp_name).is_some() || frame.nested_declarations.contains(csharp_name) {
            return Err(format!(
                "name '{csharp_name}' is already declared at {}",
                location(node)
            ));
        }

        let base = rust_name(csharp_name, role)?;
        let mut rust_name = base.clone();
        let mut suffix = 2;
        while self
            .frames
            .iter()
            .flat_map(|frame| &frame.bindings)
            .any(|binding| binding.rust_name == rust_name)
        {
            rust_name = format!("{base}_{suffix}");
            suffix += 1;
        }
        self.frames
            .last_mut()
            .expect("scope has a frame")
            .bindings
            .push(Binding {
                csharp_name,
                rust_name: rust_name.clone(),
                ty,
            });
        Ok(rust_name)
    }

    fn in_block<T>(&mut self, check: impl FnOnce(&mut Self) -> T) -> T {
        self.frames.push(Frame::default());
        let result = check(self);
        let block = self.frames.pop().expect("scope has a block");
        let enclosing = self
            .frames
            .last_mut()
            .expect("scope has an enclosing frame");
        // Later declarations in the enclosing block still conflict with nested names.
        enclosing
            .nested_declarations
            .extend(block.nested_declarations);
        enclosing.nested_declarations.extend(
            block
                .bindings
                .into_iter()
                .map(|binding| binding.csharp_name),
        );
        result
    }
}

fn single_class(root: Node) -> Result<Node, String> {
    let mut classes = Vec::new();
    for child in significant_children(root) {
        match child.kind() {
            "class_declaration" => classes.push(child),
            "namespace_declaration" | "file_scoped_namespace_declaration" => {
                return Err(
                    "namespaces are not supported; pass one class with one public static method"
                        .to_string(),
                );
            }
            "using_directive" => {
                return Err(
                    "using directives are not supported; pass one class with one public static method"
                        .to_string(),
                );
            }
            other => {
                return Err(format!(
                    "unsupported top-level item '{other}' at {}",
                    location(child)
                ));
            }
        }
    }

    match classes.as_slice() {
        [class] => {
            reject_extra_class_syntax(*class)?;
            Ok(*class)
        }
        [] => Err("expected one class with one public static method, found no class".to_string()),
        _ => Err(format!("expected one class, found {}", classes.len())),
    }
}

fn reject_extra_class_syntax(class: Node) -> Result<(), String> {
    for child in significant_children(class) {
        match child.kind() {
            "modifier" | "identifier" | "declaration_list" => {}
            "attribute_list" => {
                return Err(format!(
                    "attributes on classes are not supported at {}",
                    location(child)
                ));
            }
            "type_parameter_list" | "type_parameter_constraints_clause" => {
                return Err(format!(
                    "generic classes are not supported at {}",
                    location(child)
                ));
            }
            "base_list" => {
                return Err(format!(
                    "base classes are not supported at {}",
                    location(child)
                ));
            }
            "parameter_list" => {
                return Err(format!(
                    "primary constructors are not supported at {}",
                    location(child)
                ));
            }
            other => {
                return Err(format!(
                    "unsupported class syntax '{other}' at {}",
                    location(child)
                ));
            }
        }
    }
    Ok(())
}

fn single_method(class: Node) -> Result<Node, String> {
    let body = class.child_by_field_name("body").ok_or_else(|| {
        "expected one public static method, but the class has no body".to_string()
    })?;

    let mut methods = Vec::new();
    for member in significant_children(body) {
        match member.kind() {
            "method_declaration" => methods.push(member),
            other => {
                return Err(format!(
                    "unsupported member '{other}' at {}; only one method is supported",
                    location(member)
                ));
            }
        }
    }

    match methods.as_slice() {
        [method] => Ok(*method),
        _ => Err(format!(
            "expected exactly one method, found {}",
            methods.len()
        )),
    }
}

fn check_method(method: Node, source: &str) -> Result<CheckedMethod, String> {
    require_public_static(method, source)?;
    reject_method_extras(method)?;

    let attributes = method_attributes(method, source)?;
    let return_type = map_type(
        method
            .child_by_field_name("returns")
            .ok_or_else(|| "method is missing a return type".to_string())?,
        source,
    )?;
    let csharp_name = node_text(
        method
            .child_by_field_name("name")
            .ok_or_else(|| "method is missing a name".to_string())?,
        source,
    )
    .to_string();
    let rust_name = rust_name(&csharp_name, "method")?;
    let mut scope = Scope::new();
    let parameters = parameters(method, source, &mut scope)?;
    let body = method_body(method)?;
    let statements = check_statements(body, source, &mut scope, return_type)?;
    if !statements.iter().any(Statement::always_returns) {
        return Err(format!(
            "method does not return a value on every path at {}",
            location(body)
        ));
    }

    Ok(CheckedMethod {
        csharp_name,
        rust_name,
        parameters,
        return_type,
        attributes,
        body: statements,
    })
}

fn method_body(method: Node) -> Result<Node, String> {
    let body = method
        .child_by_field_name("body")
        .ok_or_else(|| "method must contain a body".to_string())?;
    if body.kind() == "arrow_expression_clause" {
        return Err("expression-bodied methods are not supported; use a block body".to_string());
    }
    if body.kind() != "block" {
        return Err(format!(
            "unsupported method body '{}' at {}",
            body.kind(),
            location(body)
        ));
    }
    Ok(body)
}

fn method_attributes(method: Node, source: &str) -> Result<Vec<String>, String> {
    let mut names = Vec::new();
    for child in significant_children(method) {
        if child.kind() == "attribute_list" {
            names.extend(attribute_names(child, source)?);
        }
    }
    Ok(names)
}

fn attribute_names(list: Node, source: &str) -> Result<Vec<String>, String> {
    let mut names = Vec::new();
    for child in significant_children(list) {
        match child.kind() {
            "attribute_target_specifier" => {
                return Err(format!(
                    "attribute targets are not supported at {}",
                    location(child)
                ));
            }
            "attribute" => names.push(simple_attribute_name(child, source)?),
            other => {
                return Err(format!(
                    "unsupported attribute syntax '{other}' at {}",
                    location(child)
                ));
            }
        }
    }
    if names.is_empty() {
        return Err(format!("expected an attribute name at {}", location(list)));
    }
    Ok(names)
}

fn simple_attribute_name(attribute: Node, source: &str) -> Result<String, String> {
    for child in significant_children(attribute) {
        if child.kind() == "attribute_argument_list" {
            return Err(format!(
                "attribute arguments are not supported at {}",
                location(child)
            ));
        }
    }
    let name = attribute
        .child_by_field_name("name")
        .ok_or_else(|| format!("attribute is missing a name at {}", location(attribute)))?;
    let name = concrete(name);
    if name.kind() != "identifier" {
        return Err(format!(
            "only simple attribute names are supported at {}",
            location(name)
        ));
    }
    Ok(node_text(name, source).to_string())
}

fn require_public_static(method: Node, source: &str) -> Result<(), String> {
    let mut is_public = false;
    let mut is_static = false;
    for child in significant_children(method) {
        if child.kind() != "modifier" {
            continue;
        }
        match node_text(child, source) {
            "public" => is_public = true,
            "static" => is_static = true,
            other => {
                return Err(format!(
                    "unsupported modifier '{other}' at {}",
                    location(child)
                ));
            }
        }
    }
    if is_public && is_static {
        Ok(())
    } else {
        Err(format!(
            "method must be public and static at {}",
            location(method)
        ))
    }
}

fn reject_method_extras(method: Node) -> Result<(), String> {
    for child in significant_children(method) {
        match child.kind() {
            "attribute_list" | "modifier" | "predefined_type" | "identifier" | "parameter_list"
            | "block" | "type" => {}
            "type_parameter_list" | "type_parameter_constraints_clause" => {
                return Err(format!(
                    "generic methods are not supported at {}",
                    location(child)
                ));
            }
            "explicit_interface_specifier" => {
                return Err(format!(
                    "explicit interface methods are not supported at {}",
                    location(child)
                ));
            }
            "arrow_expression_clause" => {
                return Err(
                    "expression-bodied methods are not supported; use a block body".to_string(),
                );
            }
            other => {
                return Err(format!(
                    "unsupported method syntax '{other}' at {}",
                    location(child)
                ));
            }
        }
    }
    Ok(())
}

fn parameters<'src>(
    method: Node,
    source: &'src str,
    scope: &mut Scope<'src>,
) -> Result<Vec<Parameter>, String> {
    let list = method
        .child_by_field_name("parameters")
        .ok_or_else(|| "method is missing a parameter list".to_string())?;

    let mut parameters = Vec::new();
    for child in significant_children(list) {
        if child.kind() != "parameter" {
            return Err(format!(
                "unsupported parameter syntax '{}' at {}",
                node_text(child, source).trim(),
                location(child)
            ));
        }
        parameters.push(parameter(child, source, scope)?);
    }
    Ok(parameters)
}

fn parameter<'src>(
    node: Node,
    source: &'src str,
    scope: &mut Scope<'src>,
) -> Result<Parameter, String> {
    for child in significant_children(node) {
        match child.kind() {
            "type" | "predefined_type" | "identifier" | "array_type" | "nullable_type"
            | "qualified_name" | "generic_name" | "tuple_type" | "pointer_type"
            | "implicit_type" => {}
            "modifier" => {
                return Err(format!(
                    "unsupported parameter modifier '{}' at {}",
                    node_text(child, source),
                    location(child)
                ));
            }
            "attribute_list" => {
                return Err(format!(
                    "parameter attributes are not supported at {}",
                    location(child)
                ));
            }
            "expression" | "binary_expression" | "literal" | "integer_literal" | "real_literal"
            | "boolean_literal" => {
                return Err(format!(
                    "default parameter values are not supported at {}",
                    location(child)
                ));
            }
            other => {
                return Err(format!(
                    "unsupported parameter syntax '{other}' at {}",
                    location(child)
                ));
            }
        }
    }

    let name_node = node
        .child_by_field_name("name")
        .ok_or_else(|| format!("parameter is missing a name at {}", location(node)))?;
    let primitive_type = map_type(
        node.child_by_field_name("type")
            .ok_or_else(|| format!("parameter is missing a type at {}", location(node)))?,
        source,
    )?;
    let name = scope.declare(
        node_text(name_node, source),
        primitive_type,
        "parameter",
        name_node,
    )?;
    Ok(Parameter {
        name,
        primitive_type,
    })
}

fn return_value(statement: Node) -> Result<Node, String> {
    let mut cursor = statement.walk();
    statement
        .named_children(&mut cursor)
        .find(|child| !child.is_extra())
        .ok_or_else(|| {
            format!(
                "return statement must include an expression at {}",
                location(statement)
            )
        })
}

fn check_statements<'src>(
    block: Node,
    source: &'src str,
    scope: &mut Scope<'src>,
    return_type: PrimitiveType,
) -> Result<Vec<Statement>, String> {
    significant_children(block)
        .into_iter()
        .map(|node| check_statement(node, source, scope, return_type))
        .collect()
}

fn check_statement<'src>(
    node: Node,
    source: &'src str,
    scope: &mut Scope<'src>,
    return_type: PrimitiveType,
) -> Result<Statement, String> {
    let node = concrete(node);
    match node.kind() {
        "return_statement" => {
            let value = return_value(node)?;
            let expr = check_expression(value, source, scope)?;
            expect_type(expr.ty, return_type, value)?;
            Ok(Statement::Return(expr))
        }
        "local_declaration_statement" => check_local(node, source, scope),
        "if_statement" => check_if(node, source, scope, return_type),
        "block" => scope
            .in_block(|scope| check_statements(node, source, scope, return_type))
            .map(Statement::Block),
        "expression_statement" => Err(expression_statement_error(node)),
        other => Err(unsupported_statement(other, node)),
    }
}

fn check_local<'src>(
    node: Node,
    source: &'src str,
    scope: &mut Scope<'src>,
) -> Result<Statement, String> {
    reject_local_keywords(node, source)?;
    for child in significant_children(node) {
        if child.kind() == "modifier" {
            return Err(format!(
                "unsupported local modifier '{}' at {}",
                node_text(child, source),
                location(child)
            ));
        }
    }

    let declaration = significant_children(node)
        .into_iter()
        .find(|child| child.kind() == "variable_declaration")
        .ok_or_else(|| {
            format!(
                "local declaration is missing a variable at {}",
                location(node)
            )
        })?;
    let primitive_type = map_type(
        declaration.child_by_field_name("type").ok_or_else(|| {
            format!(
                "local declaration is missing a type at {}",
                location(declaration)
            )
        })?,
        source,
    )?;

    let declarators: Vec<Node> = raw_children(declaration)
        .into_iter()
        .filter(|child| child.kind() == "variable_declarator")
        .collect();
    if declarators.is_empty() {
        return Err(format!(
            "local declaration is missing a name at {}",
            location(declaration)
        ));
    }

    let mut locals = Vec::with_capacity(declarators.len());
    for declarator in declarators {
        locals.push(check_declarator(declarator, source, scope, primitive_type)?);
    }
    Ok(Statement::Let(locals))
}

fn check_declarator<'src>(
    node: Node,
    source: &'src str,
    scope: &mut Scope<'src>,
    declared: PrimitiveType,
) -> Result<(String, Expr), String> {
    for child in raw_children(node) {
        match child.kind() {
            "tuple_pattern" => {
                return Err(format!(
                    "tuple locals are not supported at {}",
                    location(child)
                ));
            }
            "bracketed_argument_list" => {
                return Err(format!("arrays are not supported at {}", location(child)));
            }
            _ => {}
        }
    }

    let name_node = node
        .child_by_field_name("name")
        .ok_or_else(|| format!("local declaration is missing a name at {}", location(node)))?;
    let initializer = raw_children(node)
        .into_iter()
        .find(|child| child.id() != name_node.id())
        .ok_or_else(|| {
            format!(
                "uninitialized locals are not supported at {}",
                location(node)
            )
        })?;
    let typed = check_expression(initializer, source, scope)?;
    expect_type(typed.ty, declared, initializer)?;
    let name = scope.declare(node_text(name_node, source), declared, "local", name_node)?;
    Ok((name, typed))
}

fn reject_local_keywords(node: Node, source: &str) -> Result<(), String> {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.is_named() || child.is_extra() {
            continue;
        }
        let text = node_text(child, source);
        if text == "await" || text == "using" {
            return Err(format!(
                "unsupported local declaration '{text}' at {}",
                location(child)
            ));
        }
    }
    Ok(())
}

fn check_if<'src>(
    node: Node,
    source: &'src str,
    scope: &mut Scope<'src>,
    return_type: PrimitiveType,
) -> Result<Statement, String> {
    let condition = node
        .child_by_field_name("condition")
        .ok_or_else(|| format!("if statement is missing a condition at {}", location(node)))?;
    let expr = check_expression(condition, source, scope)?;
    expect_type(expr.ty, PrimitiveType::Bool, condition)?;

    let consequence = node
        .child_by_field_name("consequence")
        .ok_or_else(|| format!("if statement is missing a body at {}", location(node)))?;
    let consequence = check_branch(consequence, source, scope, return_type)?;
    let alternative = node
        .child_by_field_name("alternative")
        .map(|node| check_branch(node, source, scope, return_type))
        .transpose()?;
    Ok(Statement::If {
        condition: expr,
        consequence: Box::new(consequence),
        alternative: alternative.map(Box::new),
    })
}

fn check_branch<'src>(
    node: Node,
    source: &'src str,
    scope: &mut Scope<'src>,
    return_type: PrimitiveType,
) -> Result<Statement, String> {
    let node = concrete(node);
    if node.kind() == "local_declaration_statement" {
        return Err(format!(
            "a branch local declaration requires a block at {}",
            location(node)
        ));
    }
    check_statement(node, source, scope, return_type)
}

fn check_expression(node: Node, source: &str, scope: &Scope<'_>) -> Result<Expr, String> {
    let node = concrete(node);
    match node.kind() {
        "identifier" => {
            let written = node_text(node, source);
            let binding = scope.lookup(written).ok_or_else(|| {
                format!(
                    "unknown name '{written}' at {}; only parameters and locals are supported",
                    location(node)
                )
            })?;
            Ok(Expr {
                kind: ExprKind::Name(binding.rust_name.clone()),
                ty: binding.ty,
            })
        }
        "parenthesized_expression" => {
            let inner = check_expression(only_significant_child(node)?, source, scope)?;
            Ok(Expr {
                ty: inner.ty,
                kind: ExprKind::Group(Box::new(inner)),
            })
        }
        "binary_expression" => check_binary(node, source, scope),
        "prefix_unary_expression" => check_prefix(node, source, scope),
        "integer_literal" => check_integer(node, source, false),
        "real_literal" => check_real(node, source),
        "boolean_literal" => Ok(Expr {
            kind: ExprKind::Bool(node_text(node, source) == "true"),
            ty: PrimitiveType::Bool,
        }),
        other => Err(unsupported_expression(other, node)),
    }
}

fn check_binary(node: Node, source: &str, scope: &Scope<'_>) -> Result<Expr, String> {
    use BinaryOp::*;

    let token = node
        .child_by_field_name("operator")
        .map(|operator| node_text(operator, source))
        .ok_or_else(|| {
            format!(
                "binary expression is missing an operator at {}",
                location(node)
            )
        })?;
    let operator = match token {
        "+" => Add,
        "-" => Subtract,
        "*" => Multiply,
        "/" => Divide,
        "<" => Less,
        "<=" => LessEqual,
        ">" => Greater,
        ">=" => GreaterEqual,
        "==" => Equal,
        "!=" => NotEqual,
        "&&" => And,
        "||" => Or,
        _ => {
            return Err(format!(
                "unsupported operator '{token}' at {}",
                location(node)
            ));
        }
    };
    let left_node = node.child_by_field_name("left").ok_or_else(|| {
        format!(
            "binary expression is missing its left operand at {}",
            location(node)
        )
    })?;
    let right_node = node.child_by_field_name("right").ok_or_else(|| {
        format!(
            "binary expression is missing its right operand at {}",
            location(node)
        )
    })?;
    let left = check_expression(left_node, source, scope)?;
    let right = check_expression(right_node, source, scope)?;
    let ty = binary_type(operator, token, left.ty, right.ty, node)?;

    if ty == PrimitiveType::I32
        && let (Some(left), Some(right)) = (left.constant_integer(), right.constant_integer())
    {
        let value = match operator {
            Add => left.checked_add(right),
            Subtract => left.checked_sub(right),
            Multiply => left.checked_mul(right),
            Divide if right == 0 => {
                return Err(format!(
                    "division by zero in constant expression at {}",
                    location(node)
                ));
            }
            Divide => left.checked_div(right),
            _ => unreachable!("only arithmetic returns int"),
        };
        return checked_integer_constant(value, node);
    }

    Ok(Expr {
        kind: ExprKind::Binary(operator, Box::new(left), Box::new(right)),
        ty,
    })
}

fn binary_type(
    operator: BinaryOp,
    token: &str,
    left: PrimitiveType,
    right: PrimitiveType,
    node: Node,
) -> Result<PrimitiveType, String> {
    use BinaryOp::*;

    if left != right {
        return Err(format!(
            "cannot apply '{token}' to {} and {} at {}; implicit numeric conversions are not supported",
            left.csharp(),
            right.csharp(),
            location(node)
        ));
    }

    match operator {
        Add | Subtract | Multiply | Divide if left.is_numeric() => Ok(left),
        Less | LessEqual | Greater | GreaterEqual if left.is_numeric() => Ok(PrimitiveType::Bool),
        Equal | NotEqual => Ok(PrimitiveType::Bool),
        And | Or if left == PrimitiveType::Bool => Ok(PrimitiveType::Bool),
        _ => Err(format!(
            "cannot apply '{token}' to {} at {}",
            left.csharp(),
            location(node)
        )),
    }
}

fn check_prefix(node: Node, source: &str, scope: &Scope<'_>) -> Result<Expr, String> {
    let token = prefix_operator(node, source)?;
    let operator = match token {
        "-" => UnaryOp::Negate,
        "!" => UnaryOp::Not,
        _ => {
            return Err(format!(
                "unsupported unary operator '{token}' at {}",
                location(node)
            ));
        }
    };
    let operand = only_significant_child(node)?;
    // An immediately negated literal can spell int.MinValue without using a wider type.
    if matches!(operator, UnaryOp::Negate) && operand.kind() == "integer_literal" {
        return check_integer(operand, source, true);
    }

    let operand = check_expression(operand, source, scope)?;
    match operator {
        UnaryOp::Negate if operand.ty.is_numeric() => {
            if let Some(value) = operand.constant_integer() {
                return checked_integer_constant(value.checked_neg(), node);
            }
        }
        UnaryOp::Not if operand.ty == PrimitiveType::Bool => {}
        _ => {
            return Err(format!(
                "cannot apply '{token}' to {} at {}",
                operand.ty.csharp(),
                location(node)
            ));
        }
    }
    Ok(Expr {
        ty: operand.ty,
        kind: ExprKind::Unary(operator, Box::new(operand)),
    })
}

fn checked_integer_constant(value: Option<i32>, node: Node) -> Result<Expr, String> {
    value.map(Expr::integer).ok_or_else(|| {
        format!(
            "integer overflow in constant expression at {}",
            location(node)
        )
    })
}

fn prefix_operator<'a>(node: Node<'a>, source: &'a str) -> Result<&'a str, String> {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.is_named() || child.is_extra() {
            continue;
        }
        let text = node_text(child, source);
        if !text.is_empty() {
            return Ok(text);
        }
    }
    Err(format!(
        "unary expression is missing an operator at {}",
        location(node)
    ))
}

fn expect_type(actual: PrimitiveType, expected: PrimitiveType, node: Node) -> Result<(), String> {
    if actual == expected {
        Ok(())
    } else {
        Err(format!(
            "cannot use {} where {} is required at {}; implicit numeric conversions are not supported",
            actual.csharp(),
            expected.csharp(),
            location(node)
        ))
    }
}

fn map_type(node: Node, source: &str) -> Result<PrimitiveType, String> {
    let node = concrete(node);
    match node.kind() {
        "predefined_type" => match node_text(node, source) {
            "int" => Ok(PrimitiveType::I32),
            "float" => Ok(PrimitiveType::F32),
            "double" => Ok(PrimitiveType::F64),
            "bool" => Ok(PrimitiveType::Bool),
            "void" => Err(format!(
                "void return types are not supported at {}",
                location(node)
            )),
            other => Err(format!(
                "unsupported type '{other}' at {}; expected int, float, double, or bool",
                location(node)
            )),
        },
        "array_type" => Err(format!("arrays are not supported at {}", location(node))),
        _ => Err(format!(
            "unsupported type '{}' at {}; expected int, float, double, or bool",
            node_text(node, source).trim(),
            location(node)
        )),
    }
}

fn check_integer(node: Node, source: &str, negative: bool) -> Result<Expr, String> {
    let text = node_text(node, source);
    let unsuffixed = text.trim_end_matches(['u', 'U', 'l', 'L']);
    if unsuffixed.len() != text.len() {
        return Err(format!(
            "unsupported integer suffix '{}' in '{text}' at {}",
            &text[unsuffixed.len()..],
            location(node)
        ));
    }

    let (radix, digits) =
        if let Some(digits) = text.strip_prefix("0x").or_else(|| text.strip_prefix("0X")) {
            (16, digits)
        } else if let Some(digits) = text.strip_prefix("0b").or_else(|| text.strip_prefix("0B")) {
            (2, digits)
        } else {
            (10, text)
        };
    let value = u32::from_str_radix(&digits.replace('_', ""), radix)
        .ok()
        .map(i64::from)
        .and_then(|value| i32::try_from(if negative { -value } else { value }).ok());
    value.map(Expr::integer).ok_or_else(|| {
        format!(
            "integer literal '{text}' is outside the supported int range at {}",
            location(node)
        )
    })
}

fn check_real(node: Node, source: &str) -> Result<Expr, String> {
    let text = node_text(node, source);
    let (number, ty) = match text.chars().last() {
        Some('f' | 'F') => (&text[..text.len() - 1], PrimitiveType::F32),
        Some('d' | 'D') => (&text[..text.len() - 1], PrimitiveType::F64),
        Some('m' | 'M') => {
            return Err(format!(
                "decimal literals are not supported at {}",
                location(node)
            ));
        }
        _ => (text, PrimitiveType::F64),
    };
    if number.is_empty() {
        return Err(format!(
            "invalid real literal '{text}' at {}",
            location(node)
        ));
    }
    Ok(Expr {
        kind: ExprKind::Real(number.to_owned()),
        ty,
    })
}

fn rust_name(name: &str, role: &str) -> Result<String, String> {
    if name.starts_with('@') || !is_plain_identifier(name) {
        return Err(format!("unsupported {role} name '{name}'"));
    }
    if name == "_" {
        return Ok("_value".to_string());
    }
    let snake = to_snake_case(name);
    if RUST_KEYWORDS.contains(&snake.as_str()) {
        return Err(format!(
            "{role} name '{name}' becomes the Rust keyword '{snake}'"
        ));
    }
    Ok(snake)
}

fn is_plain_identifier(name: &str) -> bool {
    let mut chars = name.chars();
    match chars.next() {
        Some(first) if first == '_' || first.is_ascii_alphabetic() => {}
        _ => return false,
    }
    chars.all(|ch| ch == '_' || ch.is_ascii_alphanumeric())
}

fn to_snake_case(name: &str) -> String {
    let chars: Vec<char> = name.chars().collect();
    let mut snake = String::new();
    for (index, &ch) in chars.iter().enumerate() {
        if ch.is_uppercase() {
            let previous_is_lower = index > 0 && chars[index - 1].is_lowercase();
            let next_is_lower = chars.get(index + 1).is_some_and(char::is_ascii_lowercase);
            let previous_is_upper = index > 0 && chars[index - 1].is_uppercase();
            if index > 0 && (previous_is_lower || (previous_is_upper && next_is_lower)) {
                snake.push('_');
            }
            snake.extend(ch.to_lowercase());
        } else {
            snake.push(ch);
        }
    }
    snake
}

fn expression_statement_error(node: Node) -> String {
    if let Ok(inner) = only_significant_child(node)
        && inner.kind() == "assignment_expression"
    {
        return format!("reassignment is not supported at {}", location(inner));
    }
    unsupported_statement(node.kind(), node)
}

fn unsupported_statement(kind: &str, node: Node) -> String {
    let description = match kind {
        "for_statement" | "foreach_statement" | "while_statement" | "do_statement" => "loops",
        "local_function_statement" => "local functions",
        "switch_statement" => "switch statements",
        _ => {
            return format!("unsupported statement '{kind}' at {}", location(node));
        }
    };
    format!("{description} are not supported at {}", location(node))
}

fn unsupported_expression(kind: &str, node: Node) -> String {
    let description = match kind {
        "invocation_expression" => "method calls",
        "member_access_expression" => "member access",
        "element_access_expression" => "indexing",
        "cast_expression" => "casts",
        "conditional_expression" => "conditional expressions",
        "assignment_expression" => "reassignment",
        "postfix_unary_expression" => "increment and decrement",
        _ => {
            return format!("unsupported expression '{kind}' at {}", location(node));
        }
    };
    format!("{description} are not supported at {}", location(node))
}

fn syntax_error(root: Node, source: &str) -> String {
    let Some(problem) = first_problem(root) else {
        return "syntax error in C# source".to_string();
    };
    let line = problem.start_position().row + 1;
    let column = problem.start_position().column + 1;
    if problem.is_missing() {
        return format!(
            "syntax error at line {line}, column {column}: missing {}",
            problem.kind()
        );
    }
    let snippet: String = node_text(problem, source)
        .split_whitespace()
        .next()
        .unwrap_or("")
        .chars()
        .take(40)
        .collect();
    if snippet.is_empty() {
        format!("syntax error at line {line}, column {column}")
    } else {
        format!("syntax error at line {line}, column {column} near '{snippet}'")
    }
}

fn first_problem(node: Node) -> Option<Node> {
    if node.is_error() || node.is_missing() {
        return Some(node);
    }
    let mut cursor = node.walk();
    if cursor.goto_first_child() {
        loop {
            if let Some(found) = first_problem(cursor.node()) {
                return Some(found);
            }
            if !cursor.goto_next_sibling() {
                break;
            }
        }
    }
    None
}

fn significant_children(node: Node) -> Vec<Node> {
    raw_children(node).into_iter().map(concrete).collect()
}

fn raw_children(node: Node) -> Vec<Node> {
    let mut cursor = node.walk();
    node.named_children(&mut cursor)
        .filter(|child| !child.is_extra())
        .collect()
}

fn only_significant_child(node: Node) -> Result<Node, String> {
    let children = significant_children(node);
    match children.as_slice() {
        [child] => Ok(*child),
        _ => Err(format!(
            "expected one expression at {}, found {}",
            location(node),
            children.len()
        )),
    }
}

/// Skip tree-sitter supertype wrappers such as `expression` and `type`.
fn concrete(node: Node) -> Node {
    let mut current = node;
    loop {
        if !is_supertype(current.kind()) {
            return current;
        }
        let mut children = raw_children(current).into_iter();
        let Some(child) = children.next() else {
            return current;
        };
        if children.next().is_some() {
            return current;
        }
        current = child;
    }
}

fn is_supertype(kind: &str) -> bool {
    matches!(
        kind,
        "declaration"
            | "expression"
            | "non_lvalue_expression"
            | "lvalue_expression"
            | "literal"
            | "statement"
            | "type"
            | "type_declaration"
            | "pattern"
    )
}

fn location(node: Node) -> String {
    let start = node.start_position();
    format!("line {}, column {}", start.row + 1, start.column + 1)
}

fn node_text<'a>(node: Node, source: &'a str) -> &'a str {
    source.get(node.start_byte()..node.end_byte()).unwrap_or("")
}
