use crate::tcp::TcpConfig;
use crate::QueryHandler;
use rustls::ServerConfig;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio_rustls::TlsAcceptor;
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info, warn};

/// Start DoT (DNS-over-TLS) listeners on the given addresses.
pub async fn run<H: QueryHandler>(
    addrs: &[SocketAddr],
    handler: Arc<H>,
    tls_config: Arc<ServerConfig>,
    tcp_config: TcpConfig,
    cancel: CancellationToken,
) -> std::io::Result<Vec<tokio::task::JoinHandle<()>>> {
    let mut handles = Vec::new();
    let acceptor = TlsAcceptor::from(tls_config);
    let tcp_config = Arc::new(tcp_config);

    for addr in addrs {
        let listener = TcpListener::bind(addr).await?;
        info!(addr = %addr, "DoT listener started");

        let handler = Arc::clone(&handler);
        let cancel = cancel.clone();
        let acceptor = acceptor.clone();
        let tcp_config = Arc::clone(&tcp_config);

        handles.push(tokio::spawn(async move {
            dot_accept_loop(listener, handler, acceptor, tcp_config, cancel).await;
        }));
    }

    Ok(handles)
}

async fn dot_accept_loop<H: QueryHandler>(
    listener: TcpListener,
    handler: Arc<H>,
    acceptor: TlsAcceptor,
    config: Arc<TcpConfig>,
    cancel: CancellationToken,
) {
    loop {
        tokio::select! {
            _ = cancel.cancelled() => {
                info!("DoT listener shutting down");
                return;
            }
            result = listener.accept() => {
                match result {
                    Ok((stream, src)) => {
                        debug!(src = %src, "accepted DoT connection");
                        let handler = Arc::clone(&handler);
                        let acceptor = acceptor.clone();
                        let config = Arc::clone(&config);
                        let cancel = cancel.clone();

                        tokio::spawn(async move {
                            match acceptor.accept(stream).await {
                                Ok(tls_stream) => {
                                    handle_dot_connection(tls_stream, src, handler, config, cancel).await;
                                }
                                Err(e) => {
                                    debug!(src = %src, error = %e, "TLS handshake failed");
                                }
                            }
                        });
                    }
                    Err(e) => {
                        error!(error = %e, "DoT accept error");
                    }
                }
            }
        }
    }
}

async fn handle_dot_connection<H: QueryHandler>(
    mut stream: tokio_rustls::server::TlsStream<tokio::net::TcpStream>,
    src: SocketAddr,
    handler: Arc<H>,
    config: Arc<TcpConfig>,
    cancel: CancellationToken,
) {
    // DoT uses the same 2-byte length prefix framing as DNS-over-TCP
    loop {
        let mut len_buf = [0u8; 2];
        let read_result = tokio::select! {
            _ = cancel.cancelled() => return,
            _ = tokio::time::sleep(config.idle_timeout) => {
                debug!(src = %src, "DoT connection idle timeout");
                return;
            }
            result = stream.read_exact(&mut len_buf) => result,
        };

        match read_result {
            Ok(_) => {}
            Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => {
                debug!(src = %src, "DoT connection closed by client");
                return;
            }
            Err(e) => {
                warn!(src = %src, error = %e, "DoT read error");
                return;
            }
        }

        let msg_len = u16::from_be_bytes(len_buf) as usize;
        if msg_len == 0 || msg_len > 65535 {
            warn!(src = %src, len = msg_len, "invalid DoT message length");
            return;
        }

        let mut msg_buf = vec![0u8; msg_len];
        match stream.read_exact(&mut msg_buf).await {
            Ok(_) => {}
            Err(e) => {
                warn!(src = %src, error = %e, "DoT read error on message body");
                return;
            }
        }

        if let Some(response) = handler.handle_query(&msg_buf, src).await {
            let resp_len = (response.len() as u16).to_be_bytes();
            if let Err(e) = stream.write_all(&resp_len).await {
                warn!(src = %src, error = %e, "DoT write error on length");
                return;
            }
            if let Err(e) = stream.write_all(&response).await {
                warn!(src = %src, error = %e, "DoT write error on response");
                return;
            }
        }
    }
}

/// Load a TLS ServerConfig from certificate and key PEM files.
pub fn load_tls_config(
    cert_path: &std::path::Path,
    key_path: &std::path::Path,
) -> Result<Arc<ServerConfig>, Box<dyn std::error::Error>> {
    let cert_file = std::fs::File::open(cert_path)?;
    let key_file = std::fs::File::open(key_path)?;

    let certs: Vec<_> = rustls_pemfile::certs(&mut std::io::BufReader::new(cert_file))
        .collect::<Result<_, _>>()?;
    let key = rustls_pemfile::private_key(&mut std::io::BufReader::new(key_file))?
        .ok_or("no private key found in PEM file")?;

    let config = ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(certs, key)?;

    Ok(Arc::new(config))
}
