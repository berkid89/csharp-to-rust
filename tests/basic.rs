use std::process::Command;

const ADD: &str = "pub fn add(a: f32, b: f32) -> f32 {\n    a + b\n}\n";

fn transpile(source: &str) -> Result<String, String> {
    csharp_to_rust::transpile(source).map_err(|error| error.message().to_string())
}

#[test]
fn library_example_uses_question_mark() -> Result<(), Box<dyn std::error::Error>> {
    let csharp = r#"
        public static class Calculator {
            public static int Add(int a, int b) {
                return a + b;
            }
        }
    "#;

    let rust = csharp_to_rust::transpile(csharp)?;
    assert_eq!(
        rust,
        "pub fn add(a: i32, b: i32) -> i32 {\n    i32::wrapping_add(a, b)\n}\n"
    );
    Ok(())
}

#[test]
fn transpiles_add_example() {
    let source = include_str!("../examples/add.cs");
    let generated = transpile(source).unwrap();
    assert_eq!(generated, ADD);
    assert_compiles("add", &generated);
}

#[test]
fn preserves_parentheses_around_addition() {
    let source = r#"
        public class Math
        {
            public static float Compute(float a, float b, float c)
            {
                return (a + b) * c;
            }
        }
    "#;
    let expected = "pub fn compute(a: f32, b: f32, c: f32) -> f32 {\n    (a + b) * c\n}\n";
    assert_eq!(transpile(source).unwrap(), expected);
}

#[test]
fn maps_primitive_parameter_and_return_types() {
    assert_eq!(
        transpile(
            r#"
            public class Box {
                public static int Id(int value) { return value + 1; }
            }
            "#
        )
        .unwrap(),
        "pub fn id(value: i32) -> i32 {\n    i32::wrapping_add(value, 1)\n}\n"
    );
    assert_eq!(
        transpile(
            r#"
            public class Box {
                public static float Scale(float value) { return value + 2.0f; }
            }
            "#
        )
        .unwrap(),
        "pub fn scale(value: f32) -> f32 {\n    value + 2.0f32\n}\n"
    );
    assert_eq!(
        transpile(
            r#"
            public class Box {
                public static double Half(double value) { return value / 2.0; }
            }
            "#
        )
        .unwrap(),
        "pub fn half(value: f64) -> f64 {\n    value / 2.0f64\n}\n"
    );
    assert_eq!(
        transpile(
            r#"
            public class Box {
                public static bool Yes(bool value) { return true; }
            }
            "#
        )
        .unwrap(),
        "pub fn yes(value: bool) -> bool {\n    true\n}\n"
    );
}

#[test]
fn rejects_invalid_syntax() {
    let error = transpile(
        r#"
        public class Box {
            public static int Broken(int value) { return value + ; }
        }
        "#,
    )
    .unwrap_err();
    assert!(
        error.contains("syntax error"),
        "expected a syntax error, got {error}"
    );
}

#[test]
fn rejects_loops() {
    let for_error = transpile(
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
        for_error.contains("loops"),
        "expected a loop error, got {for_error}"
    );
}

#[test]
fn rejects_unsupported_type_and_multiple_methods() {
    let type_error = transpile(
        r#"
        public class Box {
            public static string Name(string value) { return value; }
        }
        "#,
    )
    .unwrap_err();
    assert!(
        type_error.contains("unsupported type 'string'"),
        "expected a type error, got {type_error}"
    );

    let count_error = transpile(
        r#"
        public class Box {
            public static int A(int value) { return value; }
            public static int B(int value) { return value; }
        }
        "#,
    )
    .unwrap_err();
    assert!(
        count_error.contains("expected exactly one method, found 2"),
        "expected a multiple-method error, got {count_error}"
    );
}

#[test]
fn cli_transpiles_add_example() {
    let manifest = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let output = Command::new(env!("CARGO_BIN_EXE_cs2rs"))
        .arg(manifest.join("examples/add.cs"))
        .output()
        .expect("failed to run cs2rs");

    assert!(
        output.status.success(),
        "cs2rs failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout is valid UTF-8");
    assert_eq!(stdout, ADD);
}

const RELU: &str = "\
pub fn relu(value: f32) -> f32 {
    if value < 0.0f32 {
        return 0.0f32;
    }
    return value;
}
";

const WEIGHTED_SUM: &str = "\
pub fn weighted_sum(a: f32, b: f32, weight: f32) -> f32 {
    let weighted: f32 = a * weight;
    return weighted + b;
}
";

#[test]
fn transpiles_relu_without_emitting_the_attribute() {
    let source = include_str!("../examples/relu.cs");
    let method = csharp_to_rust::transpile_method(source).unwrap();
    assert_eq!(method.csharp_name, "Relu");
    assert_eq!(method.rust_name, "relu");
    assert_eq!(method.attributes, vec!["GpuKernel".to_string()]);
    assert_eq!(method.return_type, csharp_to_rust::PrimitiveType::F32);
    assert_eq!(
        method.parameters,
        vec![csharp_to_rust::Parameter {
            name: "value".to_string(),
            primitive_type: csharp_to_rust::PrimitiveType::F32,
        }]
    );
    assert_eq!(method.rust_source, RELU);
    assert!(!method.rust_source.contains("GpuKernel"));
    assert_compiles("relu", &method.rust_source);
}

#[test]
fn transpiles_local_variable() {
    let source = include_str!("../examples/weighted_sum.cs");
    let method = csharp_to_rust::transpile_method(source).unwrap();
    assert_eq!(method.rust_name, "weighted_sum");
    assert_eq!(method.attributes, vec!["GpuKernel".to_string()]);
    assert_eq!(method.parameters.len(), 3);
    assert!(
        method
            .parameters
            .iter()
            .all(|parameter| parameter.primitive_type == csharp_to_rust::PrimitiveType::F32)
    );
    assert_eq!(method.rust_source, WEIGHTED_SUM);
    assert_compiles("weighted_sum", &method.rust_source);
}

#[test]
fn transpiles_if_else_and_boolean_operators() {
    let sign = transpile(
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
    )
    .unwrap();
    assert_eq!(
        sign,
        "\
pub fn sign(value: i32) -> i32 {
    if value < 0 {
        return -1;
    } else if value > 0 {
        return 1;
    } else {
        return 0;
    }
}
"
    );
    assert_compiles("sign", &sign);

    let check = transpile(
        r#"
        public class MathOperations {
            public static bool Check(int value, bool flag) {
                bool negative = value < 0 || value <= -1;
                bool positive = value > 0 && value >= 1;
                bool same = value == 0 || value != 1;
                return negative || positive || same && !flag;
            }
        }
        "#,
    )
    .unwrap();
    assert_eq!(
        check,
        "\
pub fn check(value: i32, flag: bool) -> bool {
    let negative: bool = value < 0 || value <= -1;
    let positive: bool = value > 0 && value >= 1;
    let same: bool = value == 0 || value != 1;
    return negative || positive || same && !flag;
}
"
    );
    assert_compiles("check", &check);
}

#[test]
fn records_multiple_simple_attributes_as_metadata() {
    let method = csharp_to_rust::transpile_method(
        r#"
        public class Box {
            [GpuKernel]
            [Pure]
            public static int Id(int value) { return value; }
        }
        "#,
    )
    .unwrap();
    assert_eq!(
        method.attributes,
        vec!["GpuKernel".to_string(), "Pure".to_string()]
    );
    assert!(!method.rust_source.contains('['));
}

#[test]
fn rejects_unsupported_constructs() {
    let assignment = transpile(
        r#"
        public class Box {
            public static int Set(int value) {
                value = 1;
                return value;
            }
        }
        "#,
    )
    .unwrap_err();
    assert!(
        assignment.contains("reassignment"),
        "expected a reassignment error, got {assignment}"
    );

    let call = transpile(
        r#"
        public class Box {
            public static int Call(int value) { return Abs(value); }
        }
        "#,
    )
    .unwrap_err();
    assert!(
        call.contains("method calls"),
        "expected a method-call error, got {call}"
    );

    let array = transpile(
        r#"
        public class Box {
            public static int First(int[] value) { return 1; }
        }
        "#,
    )
    .unwrap_err();
    assert!(
        array.contains("arrays"),
        "expected an array error, got {array}"
    );

    let arguments = transpile(
        r#"
        public class Box {
            [GpuKernel(1)]
            public static int Id(int value) { return value; }
        }
        "#,
    )
    .unwrap_err();
    assert!(
        arguments.contains("attribute arguments"),
        "expected an attribute error, got {arguments}"
    );

    let uninitialized = transpile(
        r#"
        public class Box {
            public static int Missing() {
                int count;
                return count;
            }
        }
        "#,
    )
    .unwrap_err();
    assert!(
        uninitialized.contains("uninitialized"),
        "expected an uninitialized-local error, got {uninitialized}"
    );
}

#[test]
fn resolves_case_sensitive_names_before_mangling() {
    let method = csharp_to_rust::transpile_method(
        r#"
        public class Example {
            public static int Bind(int Value, int value, int value_2, int _) {
                {
                    int VALUE = 3;
                    return Value * 1000 + value * 100 + value_2 * 10 + VALUE + _;
                }
            }
        }
        "#,
    )
    .unwrap();
    let names: Vec<_> = method
        .parameters
        .iter()
        .map(|param| param.name.as_str())
        .collect();
    assert_eq!(names, ["value", "value_2", "value_2_2", "_value"]);
    assert_runs(
        "name_collisions",
        &method.rust_source,
        "assert_eq!(bind(7, 2, 4, 5), 7248);",
    );
}

#[test]
fn rejects_unknown_and_duplicate_source_names() {
    for (method, message) in [
        (
            "public static int Read(int Value) { return value; }",
            "unknown name 'value'",
        ),
        (
            "public static int Read(int Value, int Value) { return Value; }",
            "already declared",
        ),
        (
            "public static int Read(int Value) { { int Value = 1; return Value; } }",
            "already declared",
        ),
        (
            "public static int Read() { { int value = 1; } int value = 2; return value; }",
            "already declared",
        ),
        (
            "public static int Read(bool flag) { if (flag) { int value = 1; } return value; }",
            "unknown name 'value'",
        ),
    ] {
        let error = transpile(&format!("public class Example {{ {method} }}")).unwrap_err();
        assert!(error.contains(message), "{method}: {error}");
    }
}

#[test]
fn groups_comparisons_on_either_side_of_equality() {
    let generated = transpile(
        r#"
        public class Example {
            public static bool Check(int a, int b, bool flag) {
                return a < b == flag && flag != a >= b;
            }
        }
        "#,
    )
    .unwrap();
    assert_runs(
        "comparison_grouping",
        &generated,
        "assert!(check(1, 2, true)); assert!(check(2, 1, false)); assert!(!check(1, 2, false));",
    );
}

#[test]
fn wraps_runtime_int_arithmetic_with_rust_overflow_checks_on() {
    let generated = transpile(
        r#"
        public class Example {
            public static bool Wrap(int max, int min) {
                int sum = max + 1;
                int difference = min - 1;
                int product = max * 2;
                int negation = -min;
                return sum == min && difference == max && product == -2 && negation == min;
            }
        }
        "#,
    )
    .unwrap();
    assert_runs(
        "wrapping_arithmetic",
        &generated,
        "assert!(wrap(i32::MAX, i32::MIN));",
    );
}

#[test]
fn normalizes_integer_literals_and_preserves_int_min() {
    let generated = transpile(
        r#"
        public class Example {
            public static bool Check(int min, int max) {
                return 0X_2A == 42 && 0B10_1010 == 42
                    && -0X8000_0000 == min && -0B10000000_00000000_00000000_00000000 == min
                    && -2_147_483_648 == min && 2_147_483_647 == max
                    && (1 + 2) * 14 == 42;
            }
        }
        "#,
    )
    .unwrap();
    assert_runs(
        "integer_literals",
        &generated,
        "assert!(check(i32::MIN, i32::MAX));",
    );
}

#[test]
fn rejects_unsupported_integer_literals_and_constant_arithmetic() {
    for (expression, message) in [
        (
            "2147483648 == 2147483648",
            "outside the supported int range",
        ),
        ("0xFFFFFFFF == 1", "outside the supported int range"),
        ("-2147483649 == 1", "outside the supported int range"),
        ("-(2147483648) == 1", "outside the supported int range"),
        ("1U == 1", "unsupported integer suffix"),
        ("2147483647 + 1 - 1 == 1", "overflow in constant expression"),
        ("-(-2147483648) == 1", "overflow in constant expression"),
        ("-2147483648 / -1 == 1", "overflow in constant expression"),
        ("1 / 0 == 1", "division by zero in constant expression"),
    ] {
        let source = format!(
            "public class Example {{ public static bool Check() {{ return {expression}; }} }}"
        );
        let error = transpile(&source).unwrap_err();
        assert!(
            error.contains(message) && error.contains("line 1, column"),
            "{expression}: {error}"
        );
    }
}

#[test]
fn preserves_runtime_int_division_and_its_failure_cases() {
    let divide = transpile(
        "public class Example { public static int Divide(int x, int y) { return x / y; } }",
    )
    .unwrap();
    let divide_by_zero = transpile(
        "public class Example { public static int DivideByZero(int x) { return x / (2 - 2); } }",
    )
    .unwrap();
    assert_runs(
        "integer_division",
        &(divide + &divide_by_zero),
        r#"
        assert_eq!(divide(-7, 3), -2);
        assert!(std::panic::catch_unwind(|| divide(i32::MIN, -1)).is_err());
        assert!(std::panic::catch_unwind(|| divide_by_zero(1)).is_err());
        "#,
    );
}

#[test]
fn checks_return_types_and_paths_before_emitting() {
    for (body, message) in [
        ("return false;", "cannot use bool where int is required"),
        (
            "if (flag) { return 1; } else { return false; }",
            "cannot use bool where int is required",
        ),
        (
            "if (flag) { return 1; }",
            "does not return a value on every path",
        ),
    ] {
        let source =
            format!("public class Example {{ public static int Check(bool flag) {{ {body} }} }}");
        let error = transpile(&source).unwrap_err();
        assert!(error.contains(message), "{body}: {error}");
    }
}

fn assert_compiles(name: &str, rust_source: &str) {
    check_rust(name, rust_source, None);
}

fn assert_runs(name: &str, rust_source: &str, assertions: &str) {
    check_rust(name, rust_source, Some(assertions));
}

fn check_rust(name: &str, rust_source: &str, assertions: Option<&str>) {
    let dir = std::env::temp_dir().join(format!("csharp-to-rust-{name}-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("create temp dir");
    let path = dir.join(format!("{name}.rs"));
    let mut compiler = Command::new("rustc");
    compiler
        .args(["--edition", "2024", "-C", "overflow-checks=yes"])
        .arg(&path);

    let executable = dir.join(format!("{name}{}", std::env::consts::EXE_SUFFIX));
    if let Some(assertions) = assertions {
        std::fs::write(
            &path,
            format!("{rust_source}\nfn main() {{\n{assertions}\n}}\n"),
        )
        .expect("write test program");
        compiler.arg("-o").arg(&executable);
    } else {
        std::fs::write(&path, rust_source).expect("write rust source");
        compiler
            .args(["--crate-type", "lib", "--emit", "metadata", "--out-dir"])
            .arg(&dir);
    }

    let compilation = compiler.output().expect("run rustc");
    let execution = (compilation.status.success() && assertions.is_some()).then(|| {
        Command::new(executable)
            .output()
            .expect("run generated program")
    });
    std::fs::remove_dir_all(dir).expect("remove temp dir");

    assert!(
        compilation.status.success(),
        "rustc failed for {name}: {}",
        String::from_utf8_lossy(&compilation.stderr)
    );
    if let Some(execution) = execution {
        assert!(
            execution.status.success(),
            "generated program failed for {name}: {}",
            String::from_utf8_lossy(&execution.stderr)
        );
    }
}
