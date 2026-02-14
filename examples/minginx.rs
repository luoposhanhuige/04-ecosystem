// it could be a proxy to a upstream
// In summary:
// This is a simple but functional load balancer/proxy that accepts connections on port 8081 and transparently relays all traffic to a service on port 8080.
// It handles multiple concurrent connections using async/await.

// Real-World Analogy: A Mail Forwarding Service
// Imagine you run a mail forwarding company:
// [Customer]  →  [Your Mail Office]  →  [Real Business]
//                  (Proxy Server)

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::{
    io,
    net::{TcpListener, TcpStream},
};
use tracing::{info, level_filters::LevelFilter, warn};
use tracing_subscriber::{fmt::Layer, layer::SubscriberExt, util::SubscriberInitExt, Layer as _};

#[derive(Serialize, Deserialize, Clone)]
struct Config {
    upstream_addr: String,
    listen_addr: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initializes tracing/logging with INFO level
    let layer = Layer::new().with_filter(LevelFilter::INFO);
    tracing_subscriber::registry().with(layer).init();
    let config = resolve_config();
    let config = Arc::new(config);
    info!("Upstream is {}", config.upstream_addr);
    info!("Listening on {}", config.listen_addr);

    // Binds a TCP listener to the configured listen address
    let listener = TcpListener::bind(&config.listen_addr).await?;

    // Enters an infinite loop accepting client connections
    loop {
        let (client, addr) = listener.accept().await?;
        info!("Accepted connection from {}", addr);
        let cloned_config = config.clone();

        // Connection handling:
        // When a client connects, it spawns an async task
        // Establishes a connection to the upstream server
        // Calls proxy() to bridge the two connections
        tokio::spawn(async move {
            let upstream = TcpStream::connect(&cloned_config.upstream_addr).await?;
            proxy(client, upstream).await?;
            Ok::<(), anyhow::Error>(())
        });
    }
    // 解释返回类型的几种写法：
    // Ok::<(), anyhow::Error>(())     // Explicit types
    // Ok(()) as Result<(), anyhow::Error>  // Alternative syntax
    // Result::<(), anyhow::Error>::Ok(())   // Full form

    #[allow(unreachable_code)]
    Ok::<(), anyhow::Error>(())
}

// proxy() function: The core logic

// Splits both TCP streams into read/write halves
// Uses tokio::io::copy() to bidirectionally forward data:
// client_read → upstream_write (client to upstream)
// upstream_read → client_write (upstream to client)
// Uses tokio::try_join!() to run both copies concurrently until either completes or errors
// Logs bytes transferred and any errors
async fn proxy(mut client: TcpStream, mut upstream: TcpStream) -> Result<()> {
    let (mut client_read, mut client_write) = client.split();
    let (mut upstream_read, mut upstream_write) = upstream.split();
    let client_to_upstream = io::copy(&mut client_read, &mut upstream_write);
    let upstream_to_client = io::copy(&mut upstream_read, &mut client_write);
    match tokio::try_join!(client_to_upstream, upstream_to_client) {
        Ok((n, m)) => info!(
            "proxied {} bytes from client to upstream, {} bytes from upstream to client",
            n, m
        ),
        Err(e) => warn!("error proxying: {:?}", e),
    }
    Ok(())
}

// Creates a config with hardcoded addresses (listening on 0.0.0.0:8081, forwarding to 0.0.0.0:8080)
// LISTEN_ADDR = 0.0.0.0:8081  (Your mail office front desk)
// UPSTREAM_ADDR = 0.0.0.0:8080  (The real business location)
fn resolve_config() -> Config {
    Config {
        upstream_addr: "0.0.0.0:8080".to_string(),
        listen_addr: "0.0.0.0:8081".to_string(),
    }
}
