mod shutdown;

use clap::Parser;
use dns_api::{build_router as build_http_router, AppState, DnsMetrics};
use dns_authority::loader;
use dns_authority::ZoneStore;
use dns_config::config::ServerConfig;
use dns_config::validation;
use dns_router::acl::AclEngine;
use dns_router::router::Router;
use std::path::PathBuf;
use std::process;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use tokio_util::sync::CancellationToken;
use tracing::{error, info};

#[derive(Parser)]
#[command(name = "dns", version, about = "A high-performance DNS server")]
struct Cli {
    /// Path to the configuration file
    #[arg(long, default_value = "/etc/dns/config.toml")]
    config: PathBuf,

    /// Validate configuration without starting the server
    #[arg(long)]
    check_config: bool,

    /// Validate a zone file without starting the server
    #[arg(long)]
    check_zone: Option<PathBuf>,
}

fn init_logging(config: &dns_config::config::LoggingConfig) {
    use tracing_subscriber::EnvFilter;

    let filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(&config.level));

    if config.format == "json" {
        tracing_subscriber::fmt()
            .with_env_filter(filter)
            .json()
            .init();
    } else {
        tracing_subscriber::fmt().with_env_filter(filter).init();
    }
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    // Handle --check-zone
    if let Some(zone_path) = &cli.check_zone {
        // Minimal logging for validation mode
        tracing_subscriber::fmt().init();
        match loader::load_zone_file(zone_path) {
            Ok(zone) => {
                println!(
                    "Zone {} is valid ({} records, serial {})",
                    zone.origin,
                    zone.record_count(),
                    zone.serial()
                );
                process::exit(0);
            }
            Err(e) => {
                eprintln!("Zone validation failed: {}", e);
                process::exit(1);
            }
        }
    }

    // Load config
    let config = match ServerConfig::from_file(&cli.config) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Failed to load config from {}: {}", cli.config.display(), e);
            process::exit(1);
        }
    };

    // Validate config
    if let Err(errors) = validation::validate(&config) {
        for error in &errors {
            eprintln!("Config validation error: {}", error);
        }
        process::exit(1);
    }

    // Handle --check-config
    if cli.check_config {
        println!("Configuration is valid");
        process::exit(0);
    }

    // Initialize logging
    init_logging(&config.logging);

    info!("starting dns server");

    // Load zones
    info!(dir = %config.zones.directory.display(), "loading zones");
    let zones = match loader::load_zone_directory(&config.zones.directory) {
        Ok(z) => z,
        Err(e) => {
            error!(error = %e, "failed to load zones");
            process::exit(1);
        }
    };
    info!(count = zones.len(), "zones loaded");

    let zone_store = Arc::new(ZoneStore::new(zones));

    // Build ACL engine
    let acl = AclEngine::new(
        config.acls.clone(),
        &config.policy.allow_recursion,
        &config.policy.allow_query,
    );

    // Build metrics
    let metrics = DnsMetrics::new();

    // Build resolver if recursion is enabled
    let resolver = if config.recursion.enabled {
        info!("recursive resolver enabled");
        Some(Arc::new(dns_resolver::Resolver::new(
            &config.recursion,
            &config.cache,
        )))
    } else {
        info!("recursive resolver disabled");
        None
    };

    // Build router
    let mut router = Router::new(Arc::clone(&zone_store), resolver.clone(), acl);

    // Add RRL if enabled
    if config.rrl.enabled {
        let rrl = Arc::new(dns_transport::rate_limit::RrlEngine::new(
            config.rrl.responses_per_second,
            config.rrl.slip,
            config.rrl.ipv4_prefix_length,
            config.rrl.ipv6_prefix_length,
        ));
        router = router.with_rrl(rrl);
        info!("response rate limiting enabled");
    }

    let router = Arc::new(router);

    // Cancellation token for shutdown
    let cancel = CancellationToken::new();

    // Start UDP listeners
    let udp_handles = match dns_transport::udp::run(
        &config.server.listen_udp,
        Arc::clone(&router),
        cancel.clone(),
    )
    .await
    {
        Ok(h) => h,
        Err(e) => {
            error!(error = %e, "failed to start UDP listeners");
            process::exit(1);
        }
    };

    // Start TCP listeners
    let tcp_handles = match dns_transport::tcp::run(
        &config.server.listen_tcp,
        Arc::clone(&router),
        dns_transport::tcp::TcpConfig::default(),
        cancel.clone(),
    )
    .await
    {
        Ok(h) => h,
        Err(e) => {
            error!(error = %e, "failed to start TCP listeners");
            process::exit(1);
        }
    };

    // Start DoT listeners (if TLS configured and DoT addresses specified)
    let mut dot_handles = Vec::new();
    if !config.server.listen_dot.is_empty() {
        if let (Some(cert_path), Some(key_path)) = (&config.tls.cert_path, &config.tls.key_path) {
            match dns_transport::dot::load_tls_config(cert_path, key_path) {
                Ok(tls_config) => {
                    match dns_transport::dot::run(
                        &config.server.listen_dot,
                        Arc::clone(&router),
                        tls_config,
                        dns_transport::tcp::TcpConfig::default(),
                        cancel.clone(),
                    )
                    .await
                    {
                        Ok(h) => dot_handles = h,
                        Err(e) => error!(error = %e, "failed to start DoT listeners"),
                    }
                }
                Err(e) => error!(error = %e, "failed to load TLS config for DoT"),
            }
        } else {
            error!("DoT listen addresses configured but TLS cert/key not provided");
        }
    }

    // Start DoH listeners
    let mut doh_handles = Vec::new();
    if !config.server.listen_doh.is_empty() {
        match dns_transport::doh::run(
            &config.server.listen_doh,
            Arc::clone(&router),
            cancel.clone(),
        )
        .await
        {
            Ok(h) => doh_handles = h,
            Err(e) => error!(error = %e, "failed to start DoH listeners"),
        }
    }

    // Build and start HTTP management API
    let ready = Arc::new(AtomicBool::new(false));
    let app_state = AppState {
        zone_store: Arc::clone(&zone_store),
        resolver: resolver.clone(),
        metrics: Arc::clone(&metrics),
        ready: ready.clone(),
        zone_directory: config.zones.directory.clone(),
    };
    let http_router = build_http_router(app_state);
    let http_addr = config.server.listen_http;
    let http_cancel = cancel.clone();
    let http_handle = tokio::spawn(async move {
        let listener = match tokio::net::TcpListener::bind(http_addr).await {
            Ok(l) => l,
            Err(e) => {
                error!(error = %e, addr = %http_addr, "failed to bind HTTP listener");
                return;
            }
        };
        info!(addr = %http_addr, "HTTP management API listening");
        axum::serve(listener, http_router)
            .with_graceful_shutdown(async move { http_cancel.cancelled().await })
            .await
            .ok();
    });

    // Mark server as ready
    ready.store(true, std::sync::atomic::Ordering::Relaxed);
    info!("server ready");

    // Spawn SIGHUP handler for zone reload
    #[cfg(unix)]
    {
        let zone_store = Arc::clone(&zone_store);
        let zone_dir = config.zones.directory.clone();
        tokio::spawn(async move {
            loop {
                shutdown::sighup_signal().await;
                info!("received SIGHUP, reloading zones");
                match zone_store.reload_all(&zone_dir) {
                    Ok(result) => {
                        info!(zones_loaded = result.zones_loaded, "zone reload complete");
                    }
                    Err(e) => {
                        error!(error = %e, "zone reload failed");
                    }
                }
            }
        });
    }

    // Wait for shutdown signal
    shutdown::shutdown_signal(cancel).await;

    info!("waiting for listeners to shut down");

    // Wait for all listener tasks to finish
    for handle in udp_handles {
        let _ = handle.await;
    }
    for handle in tcp_handles {
        let _ = handle.await;
    }
    for handle in dot_handles {
        let _ = handle.await;
    }
    for handle in doh_handles {
        let _ = handle.await;
    }
    let _ = http_handle.await;

    info!("server stopped");
}
