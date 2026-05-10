mod config;
mod platform;
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
            let port_override = parse_port_arg(args_after(&args, "proxy"));
            if let Err(e) = proxy::run(port_override) {
                eprintln!("Proxy error: {}", e);
                std::process::exit(1);
            }
        }
        Some("install") => {
            let extra = args_after(&args, "install");
            let (port, age) = parse_install_args(extra);
            let port = port.unwrap_or(2015);
            let age = age.as_deref().unwrap_or("14 days");

            if let Err(e) = platform::install(port, age) {
                eprintln!("install failed: {}", e);
                std::process::exit(1);
            }
            println!("cargo-min-publish-age installed on port {} with age {}", port, age);
        }
        Some("uninstall") => {
            if let Err(e) = platform::uninstall() {
                eprintln!("uninstall failed: {}", e);
                std::process::exit(1);
            }
            println!("cargo-min-publish-age uninstalled");
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

fn args_after<'a>(args: &'a [String], marker: &str) -> &'a [String] {
    if let Some(pos) = args.iter().position(|a| a == marker) {
        &args[pos + 1..]
    } else {
        &[]
    }
}

fn parse_port_arg(args: &[String]) -> Option<u16> {
    let mut i = 0;
    while i < args.len() {
        if args[i] == "--port" && i + 1 < args.len() {
            return args[i + 1].parse().ok();
        }
        i += 1;
    }
    None
}

fn parse_install_args(args: &[String]) -> (Option<u16>, Option<String>) {
    let mut port = None;
    let mut age = None;
    let mut i = 0;
    while i < args.len() {
        if args[i] == "--port" && i + 1 < args.len() {
            port = args[i + 1].parse().ok();
            i += 2;
        } else if args[i] == "--age" && i + 1 < args.len() {
            age = Some(args[i + 1].clone());
            i += 2;
        } else {
            i += 1;
        }
    }
    (port, age)
}
