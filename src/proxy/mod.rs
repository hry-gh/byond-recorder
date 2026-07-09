mod connection;

use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Result;
use tokio::net::TcpListener;
use tracing::{info, error};

use crate::recorder::RoundRecorder;

pub struct Config {
    pub listen_addr: SocketAddr,
    pub upstream_addr: SocketAddr,
    pub recording_dir: PathBuf,
    pub game_dir: PathBuf,
    pub round_id: String,
}

pub async fn run_with_recorder(config: Config, recorder: Arc<RoundRecorder>) -> Result<()> {
    let listener = TcpListener::bind(config.listen_addr).await?;
    info!("proxy listening on {}", config.listen_addr);
    info!("proxying to {}", config.upstream_addr);

    loop {
        let (client_stream, client_addr) = listener.accept().await?;
        info!("new connection from {}", client_addr);

        let upstream_addr = config.upstream_addr;
        let recorder = Arc::clone(&recorder);

        tokio::spawn(async move {
            if let Err(e) = connection::handle(client_stream, client_addr, upstream_addr, recorder).await {
                error!("connection {} error: {:#}", client_addr, e);
            }
        });
    }
}
