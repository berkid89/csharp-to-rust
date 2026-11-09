use std::env;
use std::fs;
use std::process::ExitCode;

fn main() -> ExitCode {
    let mut args = env::args().skip(1);
    let Some(path) = args.next() else {
        eprintln!("usage: cs2rs <file.cs>");
        return ExitCode::from(1);
    };
    if args.next().is_some() {
        eprintln!("usage: cs2rs <file.cs>");
        eprintln!("only one input file is supported");
        return ExitCode::from(1);
    }

    let source = match fs::read_to_string(&path) {
        Ok(source) => source,
        Err(error) => {
            eprintln!("failed to read {path}: {error}");
            return ExitCode::from(1);
        }
    };

    match csharp_to_rust::transpile(&source) {
        Ok(rust) => {
            print!("{rust}");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("{error}");
            ExitCode::from(1)
        }
    }
}
