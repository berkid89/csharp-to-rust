# csharp-to-rust

Translate C# source code into equivalent Rust.

csharp-to-rust is a transpiler written in Rust. It reads a small subset of C# and emits Rust that preserves the behavior of that subset. Use it as the `csharp_to_rust` library, or run the `cs2rs` command-line tool.

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

## Quick start

Install Rustup and Cargo by following the instructions at [rust-lang.org/tools/install](https://www.rust-lang.org/tools/install).

```bash
git clone https://github.com/berkid89/csharp-to-rust.git
cd csharp-to-rust
cargo run --bin cs2rs -- examples/add.cs
cargo test
```

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
examples/add.cs            arithmetic sample
examples/relu.cs           conditional sample
examples/weighted_sum.cs   local-variable sample
```

Before submitting a change:

```bash
cargo fmt
cargo clippy --all-targets -- -D warnings
```

Put translation logic in the library and add tests under `tests/` for the behavior you add. Keep `cs2rs` a thin wrapper around the library.

## License

Licensed under the Apache License, Version 2.0. See [LICENSE](LICENSE).
