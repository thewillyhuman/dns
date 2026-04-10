use crate::QueryHandler;
use axum::body::Bytes;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::routing::{get, post};
use axum::Router;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use hickory_proto::op::Message;
use hickory_proto::serialize::binary::BinDecodable;
use serde::Serialize;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio_util::sync::CancellationToken;
use tracing::info;

/// Shared state for DoH handlers.
struct DohState<H> {
    handler: Arc<H>,
    /// Default source address for DoH queries (since they arrive via HTTP).
    default_src: SocketAddr,
}

/// Start a DoH (DNS-over-HTTPS) server on the given addresses.
///
/// Runs plain HTTP — TLS termination is handled by DoT or an external reverse proxy.
/// For production deployments, place this behind a TLS-terminating load balancer.
pub async fn run<H: QueryHandler>(
    addrs: &[SocketAddr],
    handler: Arc<H>,
    cancel: CancellationToken,
) -> std::io::Result<Vec<tokio::task::JoinHandle<()>>> {
    let mut handles = Vec::new();

    for addr in addrs {
        let state = Arc::new(DohState {
            handler: Arc::clone(&handler),
            default_src: *addr,
        });

        let app = Router::new()
            .route("/dns-query", get(doh_get::<H>))
            .route("/dns-query", post(doh_post::<H>))
            .with_state(state);

        let cancel = cancel.clone();
        let listener = tokio::net::TcpListener::bind(addr).await?;
        info!(addr = %addr, "DoH listener started");

        handles.push(tokio::spawn(async move {
            axum::serve(listener, app)
                .with_graceful_shutdown(async move { cancel.cancelled().await })
                .await
                .ok();
        }));
    }

    Ok(handles)
}

/// DoH GET handler: DNS query in ?dns= query parameter (base64url-encoded wire format).
async fn doh_get<H: QueryHandler>(
    State(state): State<Arc<DohState<H>>>,
    headers: HeaderMap,
    axum::extract::Query(params): axum::extract::Query<DnsQueryParams>,
) -> Result<(StatusCode, HeaderMap, Vec<u8>), StatusCode> {
    let wire_query = URL_SAFE_NO_PAD
        .decode(&params.dns)
        .map_err(|_| StatusCode::BAD_REQUEST)?;

    process_doh_query(&state, &wire_query, &headers).await
}

/// DoH POST handler: DNS query in request body as wire format.
async fn doh_post<H: QueryHandler>(
    State(state): State<Arc<DohState<H>>>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<(StatusCode, HeaderMap, Vec<u8>), StatusCode> {
    // Verify content type
    let content_type = headers
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    if content_type != "application/dns-message" {
        return Err(StatusCode::UNSUPPORTED_MEDIA_TYPE);
    }

    process_doh_query(&state, &body, &headers).await
}

async fn process_doh_query<H: QueryHandler>(
    state: &DohState<H>,
    wire_query: &[u8],
    headers: &HeaderMap,
) -> Result<(StatusCode, HeaderMap, Vec<u8>), StatusCode> {
    let accept = headers
        .get("accept")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("application/dns-message");

    match state
        .handler
        .handle_query(wire_query, state.default_src)
        .await
    {
        Some(response) => {
            if accept.contains("application/dns-json") {
                // JSON response format
                match wire_to_json(&response) {
                    Ok(json) => {
                        let mut resp_headers = HeaderMap::new();
                        resp_headers
                            .insert("content-type", "application/dns-json".parse().unwrap());
                        Ok((StatusCode::OK, resp_headers, json.into_bytes()))
                    }
                    Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
                }
            } else {
                // Wire format response (default)
                let mut resp_headers = HeaderMap::new();
                resp_headers.insert("content-type", "application/dns-message".parse().unwrap());
                Ok((StatusCode::OK, resp_headers, response))
            }
        }
        None => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

/// Convert a wire-format DNS response to JSON (simplified RFC 8484 JSON format).
fn wire_to_json(wire: &[u8]) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    let msg = Message::from_bytes(wire)?;

    let response = DnsJsonResponse {
        status: msg.response_code().low() as u16,
        tc: msg.truncated(),
        rd: msg.recursion_desired(),
        ra: msg.recursion_available(),
        ad: msg.authentic_data(),
        cd: msg.checking_disabled(),
        question: msg
            .queries()
            .iter()
            .map(|q| DnsJsonQuestion {
                name: q.name().to_string(),
                r#type: q.query_type().into(),
            })
            .collect(),
        answer: msg
            .answers()
            .iter()
            .map(|r| DnsJsonAnswer {
                name: r.name().to_string(),
                r#type: r.record_type().into(),
                ttl: r.ttl(),
                data: r.data().to_string(),
            })
            .collect(),
    };

    Ok(serde_json::to_string(&response)?)
}

#[derive(serde::Deserialize)]
struct DnsQueryParams {
    dns: String,
}

#[derive(Serialize)]
struct DnsJsonResponse {
    #[serde(rename = "Status")]
    status: u16,
    #[serde(rename = "TC")]
    tc: bool,
    #[serde(rename = "RD")]
    rd: bool,
    #[serde(rename = "RA")]
    ra: bool,
    #[serde(rename = "AD")]
    ad: bool,
    #[serde(rename = "CD")]
    cd: bool,
    #[serde(rename = "Question")]
    question: Vec<DnsJsonQuestion>,
    #[serde(rename = "Answer")]
    answer: Vec<DnsJsonAnswer>,
}

#[derive(Serialize)]
struct DnsJsonQuestion {
    name: String,
    r#type: u16,
}

#[derive(Serialize)]
struct DnsJsonAnswer {
    name: String,
    r#type: u16,
    #[serde(rename = "TTL")]
    ttl: u32,
    data: String,
}
