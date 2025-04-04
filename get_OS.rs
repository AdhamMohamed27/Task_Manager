use std::env;

fn get_os() {
    println!("Your OS is: {}", env::consts::OS);
}

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        eprintln!("Usage: cargo run -- <command>");
        std::process::exit(1);
    }

    match args[1].as_str() {
        "get_os" => get_os(), // when running the code, type cargo run --"get_os"
        _ => {
            eprintln!("Unknown command: {}", args[1]);
            std::process::exit(1);
        }
    }
}
