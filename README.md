# csharp-to-rust

Translate a small subset of C# into equivalent Rust.

csharp-to-rust is a library other Rust programs can call, and a `cs2rs` command-line tool. It reads one public static method and emits a Rust function that preserves the behavior of that subset.

## Build status

[![CI](https://github.com/berkid89/csharp-to-rust/actions/workflows/ci.yml/badge.svg)](https://github.com/berkid89/csharp-to-rust/actions/workflows/ci.yml)

## Use the library

From another crate:

```bash
cargo add csharp-to-rust
```

```rust
use csharp_to_rust::transpile;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let csharp = r#"
        public static class Calculator {
            public static int Add(int a, int b) {
                return a + b;
            }
        }
    "#;

    let rust = transpile(csharp)?;
    println!("{rust}");

    Ok(())
}
```

That prints:

```rust
pub fn add(a: i32, b: i32) -> i32 {
    i32::wrapping_add(a, b)
}
```

`transpile` returns `Result<String, csharp_to_rust::Error>`. `Error` implements `std::error::Error`, so `?` works with `Box<dyn std::error::Error>`. `transpile_method` returns the same Rust source together with the C# name, the Rust name, the parameters, the return type, and simple attribute names.

To inspect the checked method before generating Rust, see [Using the typed IR API](#using-the-typed-ir-api).

The same program is [`examples/transpile.rs`](examples/transpile.rs):

```bash
cargo run --example transpile
```

## Using the typed IR API

`analyze_method` type-checks one supported method and returns it as a read-only value. `emit_rust` turns that value into the same Rust source `transpile` would return. Callers can read the method name, both the original C# parameter names and the Rust parameter names, attribute names, the statement tree, the expression tree, and the checked type of each expression.

The IR is owned data produced by the semantic checker. Library users can inspect it, but they cannot build or change a method that skipped checking. It is an early public API and may change in future `0.x` releases.

Downstream tools can walk the tree without generating Rust first. Attribute names are labels only. This crate records them and does not interpret them.

```rust
use csharp_to_rust::ir::{ExprKind, Statement};
use csharp_to_rust::{analyze_method, emit_rust};

fn main() -> Result<(), csharp_to_rust::Error> {
    let source = r#"
        public static class Calculator {
            [GpuKernel]
            public static float Add(float a, float b) {
                return a + b;
            }
        }
    "#;

    let method = analyze_method(source)?;
    assert_eq!(method.csharp_name(), "Add");
    assert_eq!(method.rust_name(), "add");
    assert!(matches!(&method.body()[0], Statement::Return(_)));
    if let Statement::Return(expr) = &method.body()[0] {
        assert!(matches!(expr.kind(), ExprKind::Binary(..)));
    }

    let rust = emit_rust(&method);
    assert!(rust.contains("pub fn add(a: f32, b: f32) -> f32"));
    Ok(())
}
```

The runnable sample is [`examples/ir.rs`](examples/ir.rs):

```bash
cargo run --example ir
```

## Command line

Install Rustup and Cargo by following the instructions at [rust-lang.org/tools/install](https://www.rust-lang.org/tools/install).

```bash
git clone https://github.com/berkid89/csharp-to-rust.git
cd csharp-to-rust
cargo run --bin cs2rs -- examples/add.cs
cargo test
```

`cargo install csharp-to-rust` installs the `cs2rs` binary. From this checkout, `cargo install --path .` does the same.

`examples/add.cs` prints:

```rust
pub fn add(a: f32, b: f32) -> f32 {
    a + b
}
```

`examples/relu.cs` is a method marked `[GpuKernel]`:

```csharp
[GpuKernel]
public static float Relu(float value)
{
    if (value < 0.0f)
    {
        return 0.0f;
    }

    return value;
}
```

```bash
cargo run --bin cs2rs -- examples/relu.cs
```

The attribute is not copied into the Rust function, and it does not execute anything. It is only a name a caller can read from `transpile_method`. Producing a GPU kernel, allocating GPU memory, and launching the function are outside this transpiler.

Redirect stdout to save the Rust function:

```bash
cargo run --bin cs2rs -- examples/add.cs > add.rs
```

## Currently supported

One `public static` method inside a class. The class is only a C# wrapper and is not emitted.

- Parameter and return types: `int` → `i32`, `float` → `f32`, `double` → `f64`, `bool` → `bool`.
- Simple method attributes such as `[GpuKernel]`. Names are kept as metadata and are not written into the Rust.
- Initialized local variables with an explicit primitive type, for example `float weighted = a * weight;`.
- `if`, `else`, and nested blocks, including `return` inside a branch.
- Expressions made of parameters, locals, numeric and boolean literals, parentheses, `+`, `-`, `*`, `/`, comparisons (`<`, `<=`, `>`, `>=`, `==`, `!=`), `&&`, `||`, and unary `!` and `-`.
- A public method becomes `pub fn`, and a PascalCase name becomes snake_case (`WeightedSum` → `weighted_sum`).
- Simple numeric suffixes such as `2.0f` → `2.0f32`.
- Decimal, hexadecimal, and binary `int` literals, including digit separators and `-2147483648`.

Parameters and locals are resolved by their original, case-sensitive C# names. The generated Rust uses distinct names when those spellings map to the same identifier.

Runtime `int` addition, subtraction, multiplication, and negation wrap as in default unchecked C#. Integer constant arithmetic is checked during transpilation. Runtime integer division by zero or division overflow panics in the generated Rust.

`transpile_method` returns the original C# name, the Rust name, the parameter list, the return type, attribute names, and the generated source.

Anything outside this subset produces an error.

## Development status and roadmap

csharp-to-rust is early. The subset in [Currently supported](#currently-supported) is everything it translates; any other construct fails with an error instead of emitting Rust that only looks equivalent.

The long-term goal is a general-purpose C# to Rust transpiler: one that can take larger programs and produce Rust that keeps their behavior, on its own or inside another tool. New syntax is added one subset at a time, and only after it is checked and tested.

### Done

- [x] Public typed IR. `analyze_method` returns a read-only, type-checked method, and `emit_rust` emits the same Rust as `transpile`. This API may change in `0.x` releases.

### Near term

- [ ] Reassignment, uninitialized locals, and increment or decrement
- [ ] `void` methods and expression-bodied methods
- [ ] More than one method in a class
- [ ] Method calls and member access
- [ ] Arrays and indexing
- [ ] Loops (`for`, `foreach`, `while`, `do`) and `switch`
- [ ] Casts, the conditional operator (`? :`), and implicit numeric conversions
- [ ] Attribute arguments and qualified attribute names

Namespaces, `using` directives, local functions, and other syntax outside these lists are rejected the same way.

### Later

- [ ] Classes, structs, properties, and constructors
- [ ] Enums and interfaces
- [ ] Generics
- [ ] Strings and collections
- [ ] Inheritance and polymorphism
- [ ] Exception handling
- [ ] Lambdas and LINQ
- [ ] `async` and `await`
- [ ] Translating C# references into Rust ownership
- [ ] Multiple source files and project dependencies

Contributions and feedback are welcome. [Development](#development) describes how to build, test, and submit a change.

## Development

```text
src/lib.rs                 public library API (`csharp_to_rust`)
src/ir.rs                  typed, read-only method IR
src/check.rs               C# parsing, name resolution, and validation
src/emit.rs                Rust generation from the typed IR
src/main.rs                command-line tool (`cs2rs`)
tests/                     integration tests, including `tests/ir.rs`
examples/transpile.rs      library usage sample
examples/ir.rs             typed IR sample
examples/add.cs            arithmetic sample
examples/relu.cs           conditional sample
examples/weighted_sum.cs   local-variable sample
```

Before submitting a change:

```bash
cargo fmt
cargo clippy --all-targets -- -D warnings
cargo test --all-targets
cargo test --doc
```

Put translation logic in the library and add tests under `tests/` for the behavior you add. Keep `cs2rs` a thin wrapper around the library.

### Publish

Releases go through [`.github/workflows/publish.yml`](.github/workflows/publish.yml). That workflow is separate from CI. It runs when a `v*` tag is pushed, or when you start **Publish to crates.io** from the Actions tab. A tag run checks that the tag matches `version` in `Cargo.toml`. Both paths run the tests, then publish with [trusted publishing](https://crates.io/docs/trusted-publishing).

On the crate's Settings → Trusted Publishing page, GitHub uses:

| Field | Value |
| --- | --- |
| Repository owner | `berkid89` |
| Repository name | `csharp-to-rust` |
| Workflow filename | `publish.yml` |
| Environment | leave empty |

Bump the version and commit it. Then either push a matching tag:

```bash
git tag v0.2.0
git push origin v0.2.0
```

or open the Actions tab, choose **Publish to crates.io**, and run it on the branch that contains that version. The manual form lists branches, so the workflow file has to be on the default branch before it appears there.

A published version number cannot be reused. Yank it if it must be hidden.

## License

Licensed under the Apache License, Version 2.0. See [LICENSE](LICENSE).
