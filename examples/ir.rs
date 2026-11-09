use csharp_to_rust::ir::{ExprKind, Statement};
use csharp_to_rust::{analyze_method, emit_rust};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
        public static class Calculator {
            [GpuKernel]
            public static float Add(float a, float b) {
                return a + b;
            }
        }
    "#;

    let method = analyze_method(source)?;
    println!("{} -> {}", method.csharp_name(), method.rust_name());
    println!("attributes: {:?}", method.attributes());
    for parameter in method.parameters() {
        println!(
            "parameter {} -> {}: {:?}",
            parameter.csharp_name(),
            parameter.rust_name(),
            parameter.primitive_type()
        );
    }

    let Statement::Return(expr) = &method.body()[0] else {
        return Err("expected the body to start with return".into());
    };
    let ExprKind::Binary(operator, left, right) = expr.kind() else {
        return Err("expected a binary expression".into());
    };
    println!(
        "return {operator:?} ({:?}, {:?}) : {:?}",
        left.kind(),
        right.kind(),
        expr.ty()
    );

    print!("{}", emit_rust(&method));
    Ok(())
}
