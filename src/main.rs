mod config;
mod proxy;

fn main() {
    let args: Vec<String> = std::env::args().collect();

    let subcmd = match args.len() {
        1 => None,
        _ if args[1] == "min-publish-age" => args.get(2).map(|s| s.as_str()),
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
            eprintln!("Usage: cargo min-publish-age [install|uninstall|proxy]");
            std::process::exit(1);
        }
        None => {
            println!("cargo-min-publish-age");
            println!("Run 'cargo min-publish-age install' to set up the registry proxy.");
        }
    }
}
