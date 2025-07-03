use anyhow::Result;
use std::env;

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();

    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        eprintln!("Usage: papyrust-daemon <command> [args...]");
        eprintln!("Commands:");
        eprintln!("  status");
        std::process::exit(1);
    }

    match args[1].as_str() {
        "status" => {}
        _ => {
            eprintln!("Unknown command: {}", args[1]);
            std::process::exit(1);
        }
    }

    Ok(())
}
