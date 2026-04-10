use crate::health::{self, ReadyState};
use crate::metrics::DnsMetrics;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::Json;
use axum::routing::{get, post};
use axum::Router;
use dns_authority::ZoneStore;
use dns_resolver::Resolver;
use hickory_proto::rr::Name;
use serde::Serialize;
use std::path::PathBuf;
use std::sync::Arc;
use tracing::{error, info};

/// Shared application state for all API handlers.
#[derive(Clone)]
pub struct AppState {
    pub zone_store: Arc<ZoneStore>,
    pub resolver: Option<Arc<Resolver>>,
    pub metrics: Arc<DnsMetrics>,
    pub ready: ReadyState,
    pub zone_directory: PathBuf,
}

/// Build the HTTP management API router.
pub fn build_router(state: AppState) -> Router {
    let health_router = Router::new()
        .route("/health/live", get(health::liveness))
        .route("/health/ready", get(health::readiness))
        .with_state(state.ready.clone());

    let api_router = Router::new()
        .route("/api/v1/reload", post(reload_all))
        .route("/api/v1/reload/{zone}", post(reload_zone))
        .route("/api/v1/zones", get(list_zones))
        .route("/api/v1/cache/flush", post(flush_cache))
        .route("/api/v1/cache/flush/{name}", post(flush_cache_name))
        .route("/api/v1/cache/stats", get(cache_stats))
        .route("/metrics", get(metrics_endpoint))
        .with_state(state);

    Router::new().merge(health_router).merge(api_router)
}

#[derive(Serialize)]
struct ReloadResponse {
    status: String,
    zones_loaded: usize,
    errors: Vec<String>,
}

async fn reload_all(State(state): State<AppState>) -> Json<ReloadResponse> {
    info!("API: reloading all zones");
    match state.zone_store.reload_all(&state.zone_directory) {
        Ok(result) => Json(ReloadResponse {
            status: "ok".to_string(),
            zones_loaded: result.zones_loaded,
            errors: result.errors,
        }),
        Err(e) => {
            error!(error = %e, "API: zone reload failed");
            Json(ReloadResponse {
                status: "error".to_string(),
                zones_loaded: 0,
                errors: vec![e.to_string()],
            })
        }
    }
}

async fn reload_zone(
    State(state): State<AppState>,
    Path(zone_name): Path<String>,
) -> (StatusCode, Json<ReloadResponse>) {
    info!(zone = %zone_name, "API: reloading zone");
    let zone_file = state.zone_directory.join(format!("{}.zone", zone_name));
    match state.zone_store.reload_zone(&zone_file) {
        Ok(()) => (
            StatusCode::OK,
            Json(ReloadResponse {
                status: "ok".to_string(),
                zones_loaded: 1,
                errors: Vec::new(),
            }),
        ),
        Err(e) => {
            error!(zone = %zone_name, error = %e, "API: zone reload failed");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ReloadResponse {
                    status: "error".to_string(),
                    zones_loaded: 0,
                    errors: vec![e.to_string()],
                }),
            )
        }
    }
}

#[derive(Serialize)]
struct ZoneInfo {
    name: String,
    records: usize,
    serial: u32,
}

#[derive(Serialize)]
struct ZoneListResponse {
    zones: Vec<ZoneInfo>,
}

async fn list_zones(State(state): State<AppState>) -> Json<ZoneListResponse> {
    let zone_names = state.zone_store.zone_names();
    let zones = zone_names
        .iter()
        .filter_map(|name| {
            state.zone_store.get_zone(name).map(|zone| ZoneInfo {
                name: name.to_string(),
                records: zone.record_count(),
                serial: zone.serial(),
            })
        })
        .collect();

    Json(ZoneListResponse { zones })
}

#[derive(Serialize)]
struct StatusResponse {
    status: String,
}

async fn flush_cache(State(state): State<AppState>) -> Json<StatusResponse> {
    if let Some(resolver) = &state.resolver {
        resolver.flush_cache();
        info!("API: cache flushed");
    }
    Json(StatusResponse {
        status: "ok".to_string(),
    })
}

async fn flush_cache_name(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> (StatusCode, Json<StatusResponse>) {
    match Name::from_ascii(&name) {
        Ok(dns_name) => {
            if let Some(resolver) = &state.resolver {
                resolver.flush_name(&dns_name);
                info!(name = %name, "API: cache flushed for name");
            }
            (
                StatusCode::OK,
                Json(StatusResponse {
                    status: "ok".to_string(),
                }),
            )
        }
        Err(_) => (
            StatusCode::BAD_REQUEST,
            Json(StatusResponse {
                status: "invalid name".to_string(),
            }),
        ),
    }
}

#[derive(Serialize)]
struct CacheStatsResponse {
    size: u64,
    hits: u64,
    misses: u64,
}

async fn cache_stats(State(state): State<AppState>) -> Json<CacheStatsResponse> {
    if let Some(resolver) = &state.resolver {
        let stats = resolver.cache_stats();
        Json(CacheStatsResponse {
            size: stats.size,
            hits: stats.hits,
            misses: stats.misses,
        })
    } else {
        Json(CacheStatsResponse {
            size: 0,
            hits: 0,
            misses: 0,
        })
    }
}

async fn metrics_endpoint(State(state): State<AppState>) -> String {
    state.metrics.encode()
}
