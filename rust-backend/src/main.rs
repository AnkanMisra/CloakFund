use rust_backend::{
    AppConfig, ConvexRepository, SweeperService, WatcherService, create_router, stealth,
};
use std::env;
use std::sync::Arc;
use tokio::net::TcpListener;
use tracing::{error, info};

#[tokio::main]
async fn main() {
    // Load environment variables from .env or .env.local files
    let _ = dotenvy::from_filename("../.env");
    let _ = dotenvy::dotenv();
    let _ = dotenvy::from_filename("../.env.local");
    let _ = dotenvy::from_filename(".env.local");

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
            }
        }
        "recover" => {
            if args.len() < 4 {
                eprintln!("Missing arguments for recover");
                return;
            }
            match stealth::recover_stealth_private_key(&args[2], &args[3]) {
                Ok(priv_key) => println!("Stealth Private Key: 0x{}", hex::encode(*priv_key)),
                Err(e) => eprintln!("Error: {}", e),
            }
        }
        _ => eprintln!("Unknown command: {}", args[1]),
    }
}

async fn run_server() -> anyhow::Result<()> {
    let mut config = AppConfig::from_env()?;

    info!("Initializing Convex client...");
    let convex = Arc::new(ConvexRepository::new(&config.convex).await?);

    // WATCHER_START_BLOCK env var takes priority; only fall back to checkpoint
    if let Some(start_block) = config.watcher.start_block {
        info!(
            "WATCHER_START_BLOCK override: starting from block {}",
            start_block
        );
    } else {
        info!("Fetching last watcher checkpoint...");
        if let Ok(Some(checkpoint)) = convex.get_latest_checkpoint().await {
            let resume_block = checkpoint
                .latest_processed_block
                .unwrap_or(checkpoint.start_block);
            config.watcher.start_block = Some(resume_block);
            info!("Resuming watcher from block {}", resume_block);
        }
    }

    info!("Starting watcher service...");
    let watcher = WatcherService::new(config.watcher.clone(), convex.clone());
    tokio::spawn(async move {
        if let Err(e) = watcher.start().await {
            error!("Watcher service failed: {}", e);
        }
    });

    // Create a SEPARATE Convex client for the sweeper to avoid
    // mutex contention with the watcher's frequent checkpoint updates.
    info!("Initializing dedicated sweeper Convex client...");
    let sweeper_convex = Arc::new(ConvexRepository::new(&config.convex).await?);

    info!("Starting sweeper service...");
    let mut sweeper = SweeperService::new(config.watcher.clone(), sweeper_convex);
    tokio::spawn(async move {
        if let Err(e) = sweeper.start().await {
            error!("Sweeper service failed: {}", e);
        }
    });

    info!("Starting API server on {}", config.server.bind_addr);
    let app = create_router(convex)?;
    let listener = TcpListener::bind(config.server.bind_addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
