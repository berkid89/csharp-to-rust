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

The same program is [`examples/transpile.rs`](examples/transpile.rs):

```bash
cargo run --example transpile
```

Until version 0.1.0 is on crates.io, depend on this repository instead of `cargo add`:

```toml
[dependencies]
csharp-to-rust = { git = "https://github.com/berkid89/csharp-to-rust" }
```

## Command line

Install Rustup and Cargo by following the instructions at [rust-lang.org/tools/install](https://www.rust-lang.org/tools/install).

```bash
git clone https://github.com/berkid89/csharp-to-rust.git
cd csharp-to-rust
cargo run --bin cs2rs -- examples/add.cs
cargo test
```

After the crate is published, `cargo install csharp-to-rust` installs the `cs2rs` binary. From this checkout, `cargo install --path .` does the same.

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
src/check.rs               C# parsing, name resolution, and validation
src/emit.rs                Rust generation from checked methods
src/main.rs                command-line tool (`cs2rs`)
tests/                     integration tests
examples/transpile.rs      library usage sample
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

The first release is published from your machine. [Trusted publishing](https://crates.io/docs/trusted-publishing) can be configured only after the crate exists on crates.io.

```bash
cargo login
cargo publish --dry-run
cargo publish
```

`cargo login` takes an API token from [crates.io/settings/tokens](https://crates.io/settings/tokens). Publishing `0.1.0` is what makes `cargo add csharp-to-rust` resolve for everyone. A published version number cannot be reused. Yank it if it must be hidden.

Later releases go through [`.github/workflows/publish.yml`](.github/workflows/publish.yml). That workflow is separate from CI. It runs when a `v*` tag is pushed, checks that the tag matches `version` in `Cargo.toml`, runs the tests, then publishes.

After `0.1.0` is on crates.io, open the crate's Settings → Trusted Publishing and add GitHub with:

| Field | Value |
| --- | --- |
| Repository owner | `berkid89` |
| Repository name | `csharp-to-rust` |
| Workflow filename | `publish.yml` |
| Environment | leave empty |

Then bump the version, commit it, and push a matching tag:

```bash
git tag v0.2.0
git push origin v0.2.0
```

## License

Licensed under the Apache License, Version 2.0. See [LICENSE](LICENSE).
