use crate::QueryHandler;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::UdpSocket;
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info};

/// Start UDP listeners on the given addresses.
/// Returns a vec of join handles for each listener.
pub async fn run<H: QueryHandler>(
    addrs: &[SocketAddr],
    handler: Arc<H>,
    cancel: CancellationToken,
) -> std::io::Result<Vec<tokio::task::JoinHandle<()>>> {
    let mut handles = Vec::new();

    for addr in addrs {
        let socket = UdpSocket::bind(addr).await?;
        info!(addr = %addr, "UDP listener started");

        let handler = Arc::clone(&handler);
        let cancel = cancel.clone();

        handles.push(tokio::spawn(async move {
            udp_recv_loop(socket, handler, cancel).await;
        }));
    }

    Ok(handles)
}

async fn udp_recv_loop<H: QueryHandler>(
    socket: UdpSocket,
    handler: Arc<H>,
    cancel: CancellationToken,
) {
    let socket = Arc::new(socket);
    let mut buf = vec![0u8; 4096];

    loop {
        tokio::select! {
            _ = cancel.cancelled() => {
                info!("UDP listener shutting down");
                return;
            }
            result = socket.recv_from(&mut buf) => {
                match result {
                    Ok((len, src)) => {
                        let data = buf[..len].to_vec();
                        let handler = Arc::clone(&handler);
                        let socket = Arc::clone(&socket);

                        tokio::spawn(async move {
                            debug!(src = %src, len = len, "received UDP query");
                            if let Some(response) = handler.handle_query(&data, src).await {
                                if let Err(e) = socket.send_to(&response, src).await {
                                    error!(src = %src, error = %e, "failed to send UDP response");
                                }
                            }
                        });
                    }
                    Err(e) => {
                        error!(error = %e, "UDP recv error");
                    }
                }
            }
        }
    }
}
