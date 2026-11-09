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
