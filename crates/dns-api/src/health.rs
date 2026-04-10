use axum::http::StatusCode;
use axum::response::Json;
use serde::Serialize;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// Readiness state shared between the API and the server bootstrap.
pub type ReadyState = Arc<AtomicBool>;

#[derive(Serialize)]
pub struct HealthResponse {
    pub status: String,
}

/// Liveness probe — always returns 200 after startup.
pub async fn liveness() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "alive".to_string(),
    })
}

/// Readiness probe — returns 200 if zones are loaded, 503 otherwise.
pub async fn readiness(
    axum::extract::State(ready): axum::extract::State<ReadyState>,
) -> (StatusCode, Json<HealthResponse>) {
    if ready.load(Ordering::Relaxed) {
        (
            StatusCode::OK,
            Json(HealthResponse {
                status: "ready".to_string(),
            }),
        )
    } else {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(HealthResponse {
                status: "not ready".to_string(),
            }),
        )
    }
}
