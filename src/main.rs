mod api;
mod format;
mod protocol;
mod proxy;
mod recorder;

use std::sync::Arc;

use anyhow::Result;
use tracing::info;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive("byond_recorder=info".parse()?))
        .init();

    let config = proxy::Config {
        listen_addr: "0.0.0.0:6060".parse()?,
        upstream_addr: "127.0.0.1:6061".parse()?,
        recording_dir: "./recordings".into(),
        game_dir: ".".into(),
        round_id: "default".into(),
    };

    let recorder = Arc::new(recorder::RoundRecorder::new(
        &config.recording_dir,
        &config.game_dir,
        &config.round_id,
    )?);

    let api_recorder = Arc::clone(&recorder);
    let api_addr = "127.0.0.1:6062";
    tokio::spawn(async move {
        let app = api::router(api_recorder);
        let listener = tokio::net::TcpListener::bind(api_addr).await.unwrap();
        info!("API server listening on {}", api_addr);
        axum::serve(listener, app).await.unwrap();
    });

    proxy::run_with_recorder(config, recorder).await
}
