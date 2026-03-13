use rust_backend::{create_router, stealth, AppConfig, ConvexRepository, WatcherService};
use std::env;
use std::sync::Arc;
use tokio::net::TcpListener;
use tracing::{error, info};

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        eprintln!("Usage: {} <command> <args...>", args[0]);
        eprintln!("Commands:");
        eprintln!("  serve                          Start the API server and watcher");
        eprintln!("  generate <recipient_pubkey>    Generate stealth address");
        eprintln!("  recover <recipient_priv> <ephemeral_pub>    Recover stealth private key");
        return;
    }

    match args[1].as_str() {
        "serve" => {
            if let Err(e) = run_server().await {
                error!("Server error: {}", e);
            }
        }
        "generate" => {
            if args.len() < 3 {
                eprintln!("Missing arguments for generate");
                return;
            }
            match stealth::generate_stealth_address(&args[2]) {
            Ok((addr, ephem, tag)) => {
                println!("Stealth Address: {}", addr);
                println!("Ephemeral Pubkey: {}", ephem);
                println!("View Tag: 0x{:02x}", tag);
            }
            Err(e) => eprintln!("Error: {}", e),
        },
        "recover" => {
            if args.len() < 4 {
                eprintln!("Missing arguments for recover");
                return;
            }
            match stealth::recover_stealth_private_key(&args[2], &args[3]) {
                Ok(priv_key) => println!("Stealth Private Key: {}", priv_key),
                Err(e) => eprintln!("Error: {}", e),
            }
        }
        _ => eprintln!("Unknown command: {}", args[1]),
    }
}

async fn run_server() -> anyhow::Result<()> {
    let config = AppConfig::from_env()?;

    info!("Initializing Convex client...");
    let convex = Arc::new(ConvexRepository::new(&config.convex).await?);

    info!("Starting watcher service...");
    let watcher = WatcherService::new(config.watcher.clone(), convex.clone());
    tokio::spawn(async move {
        if let Err(e) = watcher.start().await {
            error!("Watcher service failed: {}", e);
        }
    });

    info!("Starting API server on {}", config.server.bind_addr);
    let app = create_router();
    let listener = TcpListener::bind(config.server.bind_addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
