use std::fmt::Write;

use super::PrimitiveType;
use super::check::{BinaryOp, CheckedMethod, Expr, ExprKind, Statement, UnaryOp};

impl PrimitiveType {
    fn rust(self) -> &'static str {
        match self {
            Self::I32 => "i32",
            Self::F32 => "f32",
            Self::F64 => "f64",
            Self::Bool => "bool",
        }
    }
}

pub(super) fn emit(method: &CheckedMethod) -> String {
    let mut out = format!("pub fn {}(", method.rust_name);
    for (index, parameter) in method.parameters.iter().enumerate() {
        if index > 0 {
            out.push_str(", ");
        }
        write!(
            out,
            "{}: {}",
            parameter.name,
            parameter.primitive_type.rust()
        )
        .unwrap();
    }
    writeln!(out, ") -> {} {{", method.return_type.rust()).unwrap();

    if let [Statement::Return(expr)] = method.body.as_slice() {
        indent(&mut out, 1);
        emit_expr(expr, &mut out);
        out.push('\n');
    } else {
        emit_statements(&method.body, &mut out, 1);
    }
    out.push_str("}\n");
    out
}

fn emit_statements(statements: &[Statement], out: &mut String, depth: usize) {
    for statement in statements {
        emit_statement(statement, out, depth);
    }
}

fn emit_statement(statement: &Statement, out: &mut String, depth: usize) {
    match statement {
        Statement::Return(expr) => {
            indent(out, depth);
            out.push_str("return ");
            emit_expr(expr, out);
            out.push_str(";\n");
        }
        Statement::Let(locals) => {
            for (name, initializer) in locals {
                indent(out, depth);
                write!(out, "let {name}: {} = ", initializer.ty.rust()).unwrap();
                emit_expr(initializer, out);
                out.push_str(";\n");
            }
        }
        Statement::Block(_) => {
            indent(out, depth);
            emit_braced(statement, out, depth);
            out.push('\n');
        }
        Statement::If {
            condition,
            consequence,
            alternative,
        } => {
            indent(out, depth);
            emit_if(condition, consequence, alternative.as_deref(), out, depth);
            out.push('\n');
        }
    }
}

fn emit_if(
    condition: &Expr,
    consequence: &Statement,
    alternative: Option<&Statement>,
    out: &mut String,
    depth: usize,
) {
    out.push_str("if ");
    emit_expr(condition, out);
    out.push(' ');
    emit_braced(consequence, out, depth);

    if let Some(alternative) = alternative {
        out.push_str(" else ");
        match alternative {
            Statement::If {
                condition,
                consequence,
                alternative,
            } => {
                emit_if(condition, consequence, alternative.as_deref(), out, depth);
            }
            branch => emit_braced(branch, out, depth),
        }
    }
}

fn emit_braced(statement: &Statement, out: &mut String, depth: usize) {
    out.push_str("{\n");
    match statement {
        Statement::Block(statements) => emit_statements(statements, out, depth + 1),
        statement => emit_statement(statement, out, depth + 1),
    }
    indent(out, depth);
    out.push('}');
}

fn indent(out: &mut String, depth: usize) {
    for _ in 0..depth {
        out.push_str("    ");
    }
}

fn emit_expr(expr: &Expr, out: &mut String) {
    match &expr.kind {
        ExprKind::Name(name) => out.push_str(name),
        ExprKind::Integer(value) => write!(out, "{value}").unwrap(),
        ExprKind::Real(number) => {
            if number.starts_with('.') {
                out.push('0');
            }
            write!(out, "{number}{}", expr.ty.rust()).unwrap();
        }
        ExprKind::Bool(value) => write!(out, "{value}").unwrap(),
        ExprKind::Group(inner) => emit_group(inner, out),
        ExprKind::Unary(UnaryOp::Negate, operand) if expr.ty == PrimitiveType::I32 => {
            out.push_str("i32::wrapping_neg(");
            emit_argument(operand, out);
            out.push(')');
        }
        ExprKind::Unary(operator, operand) => {
            out.push(match operator {
                UnaryOp::Negate => '-',
                UnaryOp::Not => '!',
            });
            if matches!(operand.kind, ExprKind::Unary(..)) {
                emit_group(operand, out);
            } else {
                emit_expr(operand, out);
            }
        }
        ExprKind::Binary(operator, left, right) => {
            emit_binary(*operator, left, right, expr.ty, out)
        }
    }
}

fn emit_binary(operator: BinaryOp, left: &Expr, right: &Expr, ty: PrimitiveType, out: &mut String) {
    use BinaryOp::*;

    let integer_method = match (ty, operator) {
        (PrimitiveType::I32, Add) => "wrapping_add",
        (PrimitiveType::I32, Subtract) => "wrapping_sub",
        (PrimitiveType::I32, Multiply) => "wrapping_mul",
        (PrimitiveType::I32, Divide) => "checked_div",
        _ => {
            emit_infix_operand(operator, left, out);
            write!(out, " {} ", operator.token()).unwrap();
            emit_infix_operand(operator, right, out);
            return;
        }
    };
    write!(out, "i32::{integer_method}(").unwrap();
    emit_argument(left, out);
    out.push_str(", ");
    emit_argument(right, out);
    out.push(')');
    if matches!(operator, Divide) {
        // C# runtime division can fail even when the divisor is a literal.
        out.push_str(".expect(\"C# integer division by zero or overflow\")");
    }
}

fn emit_argument(mut expr: &Expr, out: &mut String) {
    // The call delimits the argument, so its outer source parentheses are redundant.
    while let ExprKind::Group(inner) = &expr.kind {
        expr = inner;
    }
    emit_expr(expr, out);
}

fn emit_infix_operand(parent: BinaryOp, operand: &Expr, out: &mut String) {
    if parent.is_comparison()
        && matches!(operand.kind, ExprKind::Binary(child, ..) if child.is_comparison())
    {
        emit_group(operand, out);
    } else {
        emit_expr(operand, out);
    }
}

fn emit_group(expr: &Expr, out: &mut String) {
    out.push('(');
    emit_expr(expr, out);
    out.push(')');
}

impl BinaryOp {
    fn token(self) -> &'static str {
        match self {
            Self::Add => "+",
            Self::Subtract => "-",
            Self::Multiply => "*",
            Self::Divide => "/",
            Self::Less => "<",
            Self::LessEqual => "<=",
            Self::Greater => ">",
            Self::GreaterEqual => ">=",
            Self::Equal => "==",
            Self::NotEqual => "!=",
            Self::And => "&&",
            Self::Or => "||",
        }
    }

    fn is_comparison(self) -> bool {
        matches!(
            self,
            Self::Less
                | Self::LessEqual
                | Self::Greater
                | Self::GreaterEqual
                | Self::Equal
                | Self::NotEqual
        )
    }
}
