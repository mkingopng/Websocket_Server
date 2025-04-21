// ============================
// openlifter-backend-bin/src/main.rs
// ============================
//! Binary entry point for the OpenLifter WebSocket server.
use std::net::SocketAddr;
use clap::Parser;
use tokio::net::TcpListener;
use tower_http::cors::{Any, CorsLayer};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use metrics_exporter_prometheus::PrometheusBuilder;
use openlifter_backend_lib::{AppState, config::{Settings, load_settings}, ws_router};

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// Path to config file
    #[arg(short, long)]
    config: Option<String>,
    
    /// Server bind address
    #[arg(short, long)]
    bind: Option<String>,
    
    /// Data directory
    #[arg(short, long)]
    data_dir: Option<String>,
    
    /// Log level
    #[arg(short, long)]
    log_level: Option<String>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Parse command line arguments
    let cli = Cli::parse();
    
    // Load settings from config file and environment
    let mut settings = load_settings()?;
    
    // Override settings with command line arguments
    if let Some(bind) = cli.bind {
        settings.bind_addr = bind.parse()?;
    }
    if let Some(data_dir) = cli.data_dir {
        settings.data_dir = data_dir.into();
    }
    if let Some(log_level) = cli.log_level {
        settings.log_level = log_level;
    }
    
    // Initialize tracing
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("RUST_LOG").unwrap_or_else(|_| settings.log_level.clone()),
        ))
        .with(tracing_subscriber::fmt::layer().json())
        .init();
    
    // Initialize metrics
    let metrics_addr = SocketAddr::from(([127, 0, 0, 1], 9091));
    let builder = PrometheusBuilder::new();
    builder.install()?;
    
    // Create app state
    let app_state = AppState::new_default()?;
    
    // Build our application with routes
    let app = ws_router::router(app_state)
        .layer(
            CorsLayer::new()
                .allow_origin(Any)
                .allow_methods(Any)
                .allow_headers(Any),
        );
    
    // Run it with hyper
    let listener = TcpListener::bind(settings.bind_addr).await?;
    tracing::info!("listening on {}", settings.bind_addr);
    
    axum::serve(listener, app.into_service()).await?;
    
    Ok(())
} 