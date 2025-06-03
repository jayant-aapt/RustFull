use std::path::Path;
use std::process::Command;
use std::time::Duration;
use warp::Filter;
use tokio::time::interval;
use futures::{SinkExt, StreamExt};  // Added both SinkExt and StreamExt
use shared_config::CONFIG;
use log;
use warp::reply::Json;
use serde_json::json;
 
// Import from the nats crate properly
use nats::publisher::NatsPublisher;
use nats::generate_nats_server_config;
 
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. First start NATS server using paths from CONFIG
    let config_path = format!("{}/nats/nats-server.conf", CONFIG.app_dir);
   
    if !Path::new(&config_path).exists() {   
        generate_nats_server_config(&config_path)?;
    }
 
    // Start NATS server asynchronously (note: now mutable)
    let mut child = tokio::process::Command::new("nats-server")
        .arg("-c")
        .arg(&config_path)
        .spawn()?;
 
    println!("NATS server started with configuration: {}", config_path);
 
    // 2. Set up Warp WebSocket for status monitoring
    let nats_status_route = warp::path!("ws" / "nats-status")
        .and(warp::ws())
        .map(|ws: warp::ws::Ws| {
            ws.on_upgrade(|websocket| async {
                handle_nats_status_ws(websocket).await
            })
        });

    // Add the /health/nats route
    let health_nats_route = warp::path!("health" / "nats")
        .and(warp::get())
        .and_then(health_nats_handler);

    // Combine with your existing routes
    let routes = nats_status_route.or(health_nats_route);

    // 3. Run both NATS server and WebSocket server
    tokio::select! {
        _ = warp::serve(routes).run(([0, 0, 0, 0], 3031)) => {
            println!("Warp server completed successfully (or panicked)");
        }
        status = child.wait() => {
            println!("NATS server process exited with status: {:?}", status);
        }
    }
 
    Ok(())
}
 
async fn handle_nats_status_ws(websocket: warp::ws::WebSocket) {
    let (mut ws_tx, wsrx) = websocket.split();
    let mut interval = interval(Duration::from_millis(500));
    let mut last_status = String::new();
 
    loop {
        interval.tick().await;
       
        let status = check_nats_status().await;
       
        // Only send if status has changed
        if status != last_status {
            if ws_tx.send(warp::ws::Message::text(status.clone())).await.is_err() {
                log::error!("Failed to send NATS status update");
                break;
            }
            last_status = status;
        }
    }
}
 
async fn check_nats_status() -> String {
    if !is_nats_process_running() {
        return "not running".to_string();
    }
 
    match try_authenticated_connection().await {
        Ok(_) => "Connected".to_string(),
        Err(e) => {
            log::error!("NATS connection error: {}", e);
            "running but connection failed".to_string()
        }
    }
}
 
fn is_nats_process_running() -> bool {
    #[cfg(windows)]
    let output = Command::new("cmd")
        .args(["/C", "tasklist | findstr nats-server.exe"])
        .output();
 
    #[cfg(not(windows))]
    let output = Command::new("pgrep")
        .arg("nats-server")
        .output();
 
    match output {
        Ok(out) => out.status.success(),
        Err(_) => false,
    }
}
 
async fn try_authenticated_connection() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let jwt = std::fs::read_to_string(&CONFIG.c_jwt_path)?;
    let nkey = std::fs::read_to_string(&CONFIG.c_nkey_path)?;
   
    let publisher = NatsPublisher::new(
        &CONFIG.nats_url,
        &jwt,
        &nkey,
        &CONFIG.ca_cert_path,
        &CONFIG.client_cert_path,
        &CONFIG.client_key_path,
    ).await?;
 
    // Add a timeout for the health check
    let timeout = tokio::time::timeout(Duration::from_secs(2), publisher.publish("health.check", &"ping")).await??;
    Ok(())
}

// Handler for /health/nats
async fn health_nats_handler() -> Result<impl warp::Reply, warp::Rejection> {
    let status = check_nats_status().await;
    let status_str = if status == "Connected" { "ok" } else { "down" };
    Ok(warp::reply::json(&json!({ "status": status_str })))
}
