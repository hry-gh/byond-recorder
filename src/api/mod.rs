use std::sync::Arc;

use axum::extract::State;
use axum::http::StatusCode;
use axum::routing::post;
use axum::{Json, Router};
use serde::Deserialize;
use tracing::{info, error};

use crate::recorder::RoundRecorder;

pub fn router(recorder: Arc<RoundRecorder>) -> Router {
    Router::new()
        .route("/round_start", post(round_start))
        .route("/round_end", post(round_end))
        .with_state(recorder)
}

#[derive(Deserialize)]
struct RoundStartRequest {
    round_id: String,
}

async fn round_start(
    State(recorder): State<Arc<RoundRecorder>>,
    Json(req): Json<RoundStartRequest>,
) -> StatusCode {
    info!("round start: {}", req.round_id);
    recorder.set_round_id(&req.round_id);
    StatusCode::OK
}

async fn round_end(
    State(recorder): State<Arc<RoundRecorder>>,
) -> StatusCode {
    info!("round end signal received");
    match recorder.end_round() {
        Ok(()) => StatusCode::OK,
        Err(e) => {
            error!("failed to end round: {:#}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        }
    }
}
