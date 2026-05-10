mod config;
mod proxy;

fn main() {
    let args: Vec<String> = std::env::args().collect();

    let subcmd = match args.len() {
        1 => None,
        _ if args[1] == "age-filter" => args.get(2).map(|s| s.as_str()),
        _ => Some(args[1].as_str()),
    };

    match subcmd {
        Some("proxy") => {
            if let Err(e) = proxy::run() {
                eprintln!("Proxy error: {}", e);
                std::process::exit(1);
            }
        }
        Some("install") => {
            eprintln!("install: not yet implemented");
            std::process::exit(1);
        }
        Some("uninstall") => {
            eprintln!("uninstall: not yet implemented");
            std::process::exit(1);
        }
        Some(other) => {
            eprintln!("Unknown subcommand: {}", other);
            eprintln!("Usage: cargo age-filter [install|uninstall|proxy]");
            std::process::exit(1);
        }
        None => {
            println!("cargo-age-filter");
            println!("Run 'cargo age-filter install' to set up the registry proxy.");
        }
    }
}
