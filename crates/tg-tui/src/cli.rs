use std::path::PathBuf;

pub struct CliArgs {
    pub portable: bool,
    pub config_path: Option<PathBuf>,
    pub data_dir: Option<PathBuf>,
    pub mock: bool,
}

impl CliArgs {
    pub fn parse() -> Self {
        let args: Vec<String> = std::env::args().collect();
        let mut portable = false;
        let mut config_path = None;
        let mut data_dir = None;
        let mut mock = false;

        let mut i = 1;
        while i < args.len() {
            match args[i].as_str() {
                "-h" | "--help" => {
                    print_help();
                    std::process::exit(0);
                }
                "-V" | "--version" => {
                    println!("TelegramTUI v{}", env!("CARGO_PKG_VERSION"));
                    std::process::exit(0);
                }
                "-p" | "--portable" => {
                    portable = true;
                }
                "--mock" => {
                    mock = true;
                }
                "-c" | "--config" => {
                    if i + 1 < args.len() {
                        config_path = Some(PathBuf::from(&args[i + 1]));
                        i += 1;
                    } else {
                        eprintln!("Error: --config requires a path argument");
                        std::process::exit(1);
                    }
                }
                "-d" | "--data-dir" => {
                    if i + 1 < args.len() {
                        data_dir = Some(PathBuf::from(&args[i + 1]));
                        i += 1;
                    } else {
                        eprintln!("Error: --data-dir requires a path argument");
                        std::process::exit(1);
                    }
                }
                unknown if unknown.starts_with('-') => {
                    eprintln!("Error: unknown option '{unknown}'");
                    eprintln!("Try 'tg-tui --help' for more information.");
                    std::process::exit(1);
                }
                _ => {}
            }
            i += 1;
        }

        Self {
            portable,
            config_path,
            data_dir,
            mock,
        }
    }
}

fn print_help() {
    println!("TelegramTUI v{}", env!("CARGO_PKG_VERSION"));
    println!("A modern cross-platform terminal Telegram client with full Vim workflow.");
    println!();
    println!("USAGE:");
    println!("    tg-tui [OPTIONS]");
    println!();
    println!("OPTIONS:");
    println!("    -h, --help               Print this help information and exit");
    println!("    -V, --version            Print version information and exit");
    println!("    -p, --portable           Force portable mode (stores all data in ./.data)");
    println!("    -c, --config <PATH>      Use custom path for config.toml");
    println!("    -d, --data-dir <PATH>    Use custom directory for session data and cache");
    println!("    --mock                   Run with offline mock backend for testing");
    println!();
    println!("DEFAULT STORAGE LOCATIONS:");
    println!("    Linux:   ~/.local/share/telegram-tui/ (data) and ~/.config/telegram-tui/ (config)");
    println!("    macOS:   ~/Library/Application Support/telegram-tui/");
    println!("    Windows: %APPDATA%\\telegram-tui\\");
    println!("    Portable: ./.data/ (auto-activated if ./.data exists or -p/--portable is passed)");
}
