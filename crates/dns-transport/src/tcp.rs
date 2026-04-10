use crate::QueryHandler;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info, warn};

/// TCP listener configuration.
pub struct TcpConfig {
    pub idle_timeout: Duration,
    pub max_connections_per_ip: usize,
}

impl Default for TcpConfig {
    fn default() -> Self {
        Self {
            idle_timeout: Duration::from_secs(10),
            max_connections_per_ip: 100,
        }
    }
}

/// Start TCP listeners on the given addresses.
pub async fn run<H: QueryHandler>(
    addrs: &[SocketAddr],
    handler: Arc<H>,
    config: TcpConfig,
    cancel: CancellationToken,
) -> std::io::Result<Vec<tokio::task::JoinHandle<()>>> {
    let mut handles = Vec::new();
    let config = Arc::new(config);

    for addr in addrs {
        let listener = TcpListener::bind(addr).await?;
        info!(addr = %addr, "TCP listener started");

        let handler = Arc::clone(&handler);
        let cancel = cancel.clone();
        let config = Arc::clone(&config);

        handles.push(tokio::spawn(async move {
            tcp_accept_loop(listener, handler, config, cancel).await;
        }));
    }

    Ok(handles)
}

async fn tcp_accept_loop<H: QueryHandler>(
    listener: TcpListener,
    handler: Arc<H>,
    config: Arc<TcpConfig>,
    cancel: CancellationToken,
) {
    loop {
        tokio::select! {
            _ = cancel.cancelled() => {
                info!("TCP listener shutting down");
                return;
            }
            result = listener.accept() => {
                match result {
                    Ok((stream, src)) => {
                        debug!(src = %src, "accepted TCP connection");
                        let handler = Arc::clone(&handler);
                        let config = Arc::clone(&config);
                        let cancel = cancel.clone();

                        tokio::spawn(async move {
                            handle_tcp_connection(stream, src, handler, config, cancel).await;
                        });
                    }
                    Err(e) => {
                        error!(error = %e, "TCP accept error");
                    }
                }
            }
        }
    }
}

async fn handle_tcp_connection<H: QueryHandler>(
    mut stream: tokio::net::TcpStream,
    src: SocketAddr,
    handler: Arc<H>,
    config: Arc<TcpConfig>,
    cancel: CancellationToken,
) {
    loop {
        // Read 2-byte length prefix (RFC 7766)
        let mut len_buf = [0u8; 2];
        let read_result = tokio::select! {
            _ = cancel.cancelled() => return,
            _ = tokio::time::sleep(config.idle_timeout) => {
                debug!(src = %src, "TCP connection idle timeout");
                return;
            }
            result = stream.read_exact(&mut len_buf) => result,
        };

        match read_result {
            Ok(_) => {}
            Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => {
                debug!(src = %src, "TCP connection closed by client");
                return;
            }
            Err(e) => {
                warn!(src = %src, error = %e, "TCP read error");
                return;
            }
        }

        let msg_len = u16::from_be_bytes(len_buf) as usize;
        if msg_len == 0 || msg_len > 65535 {
            warn!(src = %src, len = msg_len, "invalid TCP message length");
            return;
        }

        // Read the DNS message
        let mut msg_buf = vec![0u8; msg_len];
        match stream.read_exact(&mut msg_buf).await {
            Ok(_) => {}
            Err(e) => {
                warn!(src = %src, error = %e, "TCP read error on message body");
                return;
            }
        }

        // Handle query
        if let Some(response) = handler.handle_query(&msg_buf, src).await {
            let resp_len = (response.len() as u16).to_be_bytes();
            if let Err(e) = stream.write_all(&resp_len).await {
                warn!(src = %src, error = %e, "TCP write error on length");
                return;
            }
            if let Err(e) = stream.write_all(&response).await {
                warn!(src = %src, error = %e, "TCP write error on response");
                return;
            }
        }
    }
}
