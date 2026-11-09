use csharp_to_rust::ir::{BinaryOp, ExprKind, Statement};
use csharp_to_rust::{PrimitiveType, analyze_method, emit_rust, transpile, transpile_method};

#[test]
fn analyzes_attributed_addition() {
    let method = analyze_method(
        r#"
        public static class Calculator {
            [GpuKernel]
            public static float Add(float a, float b) {
                return a + b;
            }
        }
        "#,
    )
    .unwrap();

    assert_eq!(method.csharp_name(), "Add");
    assert_eq!(method.rust_name(), "add");
    assert_eq!(method.attributes(), &["GpuKernel".to_string()]);
    assert_eq!(method.return_type(), PrimitiveType::F32);
    assert_eq!(method.parameters().len(), 2);
    assert_eq!(method.parameters()[0].csharp_name(), "a");
    assert_eq!(method.parameters()[0].rust_name(), "a");
    assert_eq!(method.parameters()[0].primitive_type(), PrimitiveType::F32);
    assert_eq!(method.parameters()[1].csharp_name(), "b");
    assert_eq!(method.parameters()[1].rust_name(), "b");
    assert_eq!(method.parameters()[1].primitive_type(), PrimitiveType::F32);

    let Statement::Return(expr) = &method.body()[0] else {
        panic!("expected a return, got {:?}", method.body());
    };
    assert_eq!(expr.ty(), PrimitiveType::F32);
    let ExprKind::Binary(BinaryOp::Add, left, right) = expr.kind() else {
        panic!("expected addition, got {:?}", expr.kind());
    };
    assert_eq!(left.ty(), PrimitiveType::F32);
    assert_eq!(right.ty(), PrimitiveType::F32);
    assert!(matches!(left.kind(), ExprKind::Name(name) if name == "a"));
    assert!(matches!(right.kind(), ExprKind::Name(name) if name == "b"));
}

#[test]
fn analyzes_relu_if() {
    let method = analyze_method(include_str!("../examples/relu.cs")).unwrap();
    assert_eq!(method.csharp_name(), "Relu");
    assert_eq!(method.body().len(), 2);

    let Statement::If {
        condition,
        consequence,
        alternative,
    } = &method.body()[0]
    else {
        panic!("expected an if, got {:?}", method.body()[0]);
    };
    assert!(alternative.is_none());
    assert_eq!(condition.ty(), PrimitiveType::Bool);
    let ExprKind::Binary(BinaryOp::Less, left, right) = condition.kind() else {
        panic!("expected a comparison, got {:?}", condition.kind());
    };
    assert!(matches!(left.kind(), ExprKind::Name(name) if name == "value"));
    assert_eq!(left.ty(), PrimitiveType::F32);
    assert!(matches!(right.kind(), ExprKind::Real(number) if number == "0.0"));
    assert_eq!(right.ty(), PrimitiveType::F32);

    let Statement::Block(statements) = consequence.as_ref() else {
        panic!("expected a braced branch, got {consequence:?}");
    };
    let Statement::Return(returned) = &statements[0] else {
        panic!("expected a return from the branch, got {statements:?}");
    };
    assert_eq!(returned.ty(), PrimitiveType::F32);
    assert!(matches!(returned.kind(), ExprKind::Real(number) if number == "0.0"));

    let Statement::Return(returned) = &method.body()[1] else {
        panic!("expected a final return, got {:?}", method.body()[1]);
    };
    assert_eq!(returned.ty(), PrimitiveType::F32);
    assert!(matches!(returned.kind(), ExprKind::Name(name) if name == "value"));
}

#[test]
fn analyzes_initialized_local() {
    let method = analyze_method(include_str!("../examples/weighted_sum.cs")).unwrap();
    assert_eq!(method.body().len(), 2);

    let Statement::Let(locals) = &method.body()[0] else {
        panic!("expected a local, got {:?}", method.body()[0]);
    };
    assert_eq!(locals.len(), 1);
    assert_eq!(locals[0].0, "weighted");
    assert_eq!(locals[0].1.ty(), PrimitiveType::F32);
    let ExprKind::Binary(BinaryOp::Multiply, left, right) = locals[0].1.kind() else {
        panic!("expected a product, got {:?}", locals[0].1.kind());
    };
    assert!(matches!(left.kind(), ExprKind::Name(name) if name == "a"));
    assert_eq!(left.ty(), PrimitiveType::F32);
    assert!(matches!(right.kind(), ExprKind::Name(name) if name == "weight"));
    assert_eq!(right.ty(), PrimitiveType::F32);

    let Statement::Return(expr) = &method.body()[1] else {
        panic!("expected a return, got {:?}", method.body()[1]);
    };
    assert_eq!(expr.ty(), PrimitiveType::F32);
    assert!(matches!(
        expr.kind(),
        ExprKind::Binary(BinaryOp::Add, left, right)
            if matches!(left.kind(), ExprKind::Name(name) if name == "weighted")
            && matches!(right.kind(), ExprKind::Name(name) if name == "b")
    ));
}

#[test]
fn keeps_csharp_parameter_names_and_rust_names() {
    let method = analyze_method(
        r#"
        public class Box {
            public static float Scale(float InputScale) { return InputScale; }
        }
        "#,
    )
    .unwrap();

    assert_eq!(method.parameters()[0].csharp_name(), "InputScale");
    assert_eq!(method.parameters()[0].rust_name(), "input_scale");
    let Statement::Return(expr) = &method.body()[0] else {
        panic!("expected a return, got {:?}", method.body());
    };
    assert!(matches!(expr.kind(), ExprKind::Name(name) if name == "input_scale"));
}

#[test]
fn emit_rust_matches_transpile() {
    let sources = [
        include_str!("../examples/add.cs"),
        include_str!("../examples/relu.cs"),
        include_str!("../examples/weighted_sum.cs"),
        r#"
        public class MathOperations {
            public static int Sign(int value) {
                if (value < 0) {
                    return -1;
                } else if (value > 0) {
                    return 1;
                } else {
                    return 0;
                }
            }
        }
        "#,
    ];
    for source in sources {
        let method = analyze_method(source).unwrap();
        assert_eq!(emit_rust(&method), transpile(source).unwrap());
    }
}

#[test]
fn transpile_method_metadata_matches_the_analyzed_method() {
    let source = include_str!("../examples/relu.cs");
    let method = analyze_method(source).unwrap();
    let transpiled = transpile_method(source).unwrap();

    assert_eq!(transpiled.csharp_name, "Relu");
    assert_eq!(transpiled.rust_name, "relu");
    assert_eq!(transpiled.return_type, PrimitiveType::F32);
    assert_eq!(transpiled.attributes, ["GpuKernel".to_string()]);
    assert_eq!(
        transpiled.parameters,
        vec![csharp_to_rust::Parameter {
            name: "value".to_string(),
            primitive_type: PrimitiveType::F32,
        }]
    );
    assert_eq!(
        transpiled.rust_source,
        "\
pub fn relu(value: f32) -> f32 {
    if value < 0.0f32 {
        return 0.0f32;
    }
    return value;
}
"
    );

    assert_eq!(transpiled.csharp_name, method.csharp_name());
    assert_eq!(transpiled.rust_name, method.rust_name());
    assert_eq!(transpiled.return_type, method.return_type());
    assert_eq!(transpiled.attributes, method.attributes());
    assert_eq!(transpiled.rust_source, emit_rust(&method));
    assert_eq!(transpiled.parameters.len(), method.parameters().len());
    for (transpiled_parameter, parameter) in transpiled.parameters.iter().zip(method.parameters()) {
        assert_eq!(transpiled_parameter.name, parameter.rust_name());
        assert_eq!(
            transpiled_parameter.primitive_type,
            parameter.primitive_type()
        );
    }
}

#[test]
fn invalid_source_returns_an_error() {
    let syntax = analyze_method(
        "public class Box { public static int Broken(int value) { return value + ; } }",
    )
    .unwrap_err();
    assert!(
        syntax.message().contains("syntax error"),
        "expected a syntax error, got {syntax}"
    );

    let unsupported = analyze_method(
        r#"
        public class Box {
            public static int Sum(int value) {
                for (int i = 0; i < value; i++) { return i; }
            }
        }
        "#,
    )
    .unwrap_err();
    assert!(
        unsupported.message().contains("loops"),
        "expected a loop error, got {unsupported}"
    );
}
