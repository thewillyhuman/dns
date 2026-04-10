use tokio::signal;
use tokio_util::sync::CancellationToken;
use tracing::info;

/// Wait for a shutdown signal (SIGTERM, SIGINT) and cancel the token.
pub async fn shutdown_signal(cancel: CancellationToken) {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("failed to install SIGTERM handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {
            info!("received SIGINT, shutting down");
        }
        _ = terminate => {
            info!("received SIGTERM, shutting down");
        }
    }

    cancel.cancel();
}

/// Wait for SIGHUP signal (used for zone reload).
#[cfg(unix)]
pub async fn sighup_signal() {
    let mut sig = signal::unix::signal(signal::unix::SignalKind::hangup())
        .expect("failed to install SIGHUP handler");
    sig.recv().await;
}
