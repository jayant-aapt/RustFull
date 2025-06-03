use tracing::{info}; 
use tokio::signal;
use serde::Serialize;
use tokio::io::AsyncBufReadExt; 
use futures::StreamExt; 
use base64::{engine::general_purpose, Engine as _};
use agent_lib; 
use tokio_tungstenite::accept_async;
use tokio::net::{TcpListener, TcpStream};
use futures_util::SinkExt;
use std::sync::Arc;
use tokio::sync::broadcast;
use serde_json::json;
use std::time::Duration;
use shared_config::CONFIG;
use axum::{Router, routing::{post, get}, Json};
use std::sync::atomic::{AtomicBool, Ordering};
use serde::Deserialize;
use tokio::spawn;
use hyper::Server;
use axum::serve;
use warp::Filter;
use tracing_appender::rolling;
use tracing_subscriber::fmt::writer::MakeWriterExt;



mod key_utils;
use key_utils::KeyManager;

use nats::publisher::NatsPublisher;
use nats::subscriber::NatsSubscriber;
use models_database::db::{
    establish_connection, get_agent_details
};
use async_nats::Client;
use hostname;
use sys_info;

#[derive(Serialize,Debug)]
struct MasterKeyPayload {
    master_key: String,
    hostname: String,
    os: String,
    os_version: String,
}

async fn setup_nats_client(master_key: Vec<u8>, running: Arc<AtomicBool>) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    
    // === PUBLISHER SETUP ===
    let publisher = NatsPublisher::new(
        &CONFIG.nats_url,
        &std::fs::read_to_string(&CONFIG.c_jwt_path)?,
        &std::fs::read_to_string(&CONFIG.c_nkey_path)?,
        
        &CONFIG.ca_cert_path,
        &CONFIG.client_cert_path,
        &CONFIG.client_key_path,
    )
    .await?;

     // === SUBSCRIBER SETUP ===
    let subscriber = NatsSubscriber::new(
        &CONFIG.nats_url,
        &std::fs::read_to_string(&CONFIG.c_jwt_path)?,
        &std::fs::read_to_string(&CONFIG.c_nkey_path)?,
        &CONFIG.ca_cert_path,
        &CONFIG.client_cert_path,
        &CONFIG.client_key_path,
    )
    .await?;

    println!("CONFIG.c_nkey_path: {}", &CONFIG.c_nkey_path);

    // Publish master_key to "master_key"
    let payload = MasterKeyPayload {
        master_key: general_purpose::STANDARD.encode(&master_key),
        hostname : hostname::get()?.to_string_lossy().to_string(),
        os : format!("{} {}", sys_info::os_type()?, sys_info::os_release()?),
        os_version: sys_info::os_release()?
    };
    publisher.publish("master.key", &payload).await?;
    info!("Master key published to NATs........... ");

    // Subscribe to bridge.response topic and handle it
    let client = subscriber.client().clone();
    let subscribe_for_sacn = subscriber.client().clone(); 
    
    let pub_clone1 = publisher.clone(); // Clone for move into async
    let pub_clone2 = publisher.clone(); // For scan topic handler

    let mut sub = subscriber.client().subscribe("bridge.response".to_string()).await?;
    tokio::spawn(async move {
        while let Some(msg) = sub.next().await {
            let payload = String::from_utf8_lossy(&msg.payload);
            let mut conn = establish_connection(&CONFIG.db_path);
            
            if get_agent_details(&mut conn).is_some() {
                println!("[INFO] Device details stored in database. Skipping the collecting agent data ");
                info!("Skipping the collecting agent data ");
                start_monitoring(client.clone(), running.clone(), publisher.clone()).await;
            } else {
                println!("[INFO] Device details not found in database. Collecting the agent data...................");
                info!("Device details not found in database. Collecting the agent data...................");
            }
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&payload) {
                if json.get("status") == Some(&serde_json::Value::String("ok".to_string())) 
                || json.get("message") == Some(&serde_json::Value::String("Token is already exists".to_string()))
                {
                    info!("Collecting the agent data...................");
                    match agent_lib::agent_data() {
                        Ok(agent_data) => { 
                            if let Err(e) = pub_clone1.publish("agent.data", &agent_data).await {
                                eprintln!("Failed to publish agent data: {e}");
                            }                            

                            println!("Waiting for agent response...................");
                            let mut agent_response_sub = match subscriber.client().subscribe("agent.response".to_string()).await {
                                Ok(sub) => sub,
                                Err(e) => {
                                    eprintln!("Failed to subscribe to agent.response: {e}");
                                    return;
                                }
                            };

                            while let Some(msg) = agent_response_sub.next().await {
                                let payload = String::from_utf8_lossy(&msg.payload);
                                println!("Agent response: {}", payload);

                                if payload.contains("Data stored successfully") {
                                    println!("[INFO] Valid response received");

                                    start_monitoring(client.clone(), running.clone(), pub_clone1.clone()).await; // <-- FIX: add running.clone()
                                } else {
                                    eprintln!("[WARN] Unexpected response: {}", payload);
                                }
                            }
                        }
                        Err(e) => eprintln!("Failed to collect agent data: {e}"),
                    }
                }
            }
            else {
                eprintln!("[ERROR] Failed to parse JSON response: {}", payload);
            }
        }
    });

//handling the scan the new added topic
tokio::spawn(async move {
    let mut new_sub = match subscribe_for_sacn.subscribe("scan.>".to_string()).await {
        Ok(sub) => sub,
        Err(e) => {
            eprintln!("Failed to subscribe to scan.partition: {e}");
            return;
        }
    };

    while let Some(msg) = new_sub.next().await {
        let payload = String::from_utf8_lossy(&msg.payload);
        println!("Received scan request: {}", payload);

        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&payload) {
            if let Some(action) = json.get("action").and_then(|v| v.as_str()) {
                let uuid_value = json.get("uuid").and_then(|v| v.as_str()).unwrap_or("");
                match action {
                    "disk" | "partition" => {
                        info!("Scanning disk............................................");
                        match agent_lib::scan_disk(action) {
                            Ok(disk) => send_scan_response(&pub_clone2, action, uuid_value, disk).await,
                            Err(e) => eprintln!("Failed to scan disk: {e}"),
                        }
                    },
                    "nic" => {
                        info!("Scanning nic details............................................");
                        match agent_lib::scan_nic(action) {
                            Ok(nic_data) => send_scan_response(&pub_clone2, action, uuid_value, nic_data).await,
                            Err(e) => eprintln!("Failed to scan disk: {e}"),
                        }
                    }
                    _ => {
                        eprintln!("Unknown action received: {}", action);
                    }
                }
            }
        }
    }
});

Ok(())
}

async fn send_scan_response<T: serde::Serialize>(publisher: &NatsPublisher,   action: &str,uuid: &str ,data: T) {
    let original_json = serde_json::json!(data);
    
        let message_json = serde_json::json!({
            "uuid": uuid,
            "result": original_json,
            "action": action
        });
    
            if let Err(e) = publisher.publish(&format!("send.scan.{}", action), &message_json).await {
                eprintln!("Failed to publish agent data: {e}");
            }
       


}


async fn start_monitoring(client: Client, running: Arc<AtomicBool>, publisher: NatsPublisher) {
    // Publish monitoring running status to NATS
    let _ = publisher.publish("monitoring.status", &serde_json::json!({"status": "running"})).await;
    tracing::info!("[NATS] Published monitoring.status: running");

    println!("Type 'scan' to start collecting monitoring data:");
    let mut input = String::new();
    let mut stdin = tokio::io::BufReader::new(tokio::io::stdin());
    if let Err(e) = stdin.read_line(&mut input).await {
        eprintln!("Failed to read input: {e}");
        // Publish stopped status on error
        let _ = publisher.publish("monitoring.status", &serde_json::json!({"status": "stopped"})).await;
        tracing::info!("[NATS] Published monitoring.status: stopped (input error)");
        return;
    }

    if input.trim().eq_ignore_ascii_case("scan") {
        println!("Collecting the monitoring data...................");
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(1));
        let mut data_queue: Vec<String> = Vec::new();
        loop {
            interval.tick().await;
            if !running.load(Ordering::SeqCst) {
                tokio::time::sleep(Duration::from_secs(1)).await;
                continue;
            }
            match agent_lib::monitor_data() {
                Ok(monitor_data) => {
                    data_queue.push(monitor_data);
                    if data_queue.len() == 5 {
                        let payload = format!("[{}]", data_queue.join(","));
                        if let Err(e) = client
                            .publish("monitor.data".to_string(), payload.into_bytes().into())
                            .await
                        {
                            eprintln!("Failed to publish batch: {e}");
                        } else {
                            println!("[INFO] Sent 5-point batch to bridge");
                            data_queue.clear();
                        }
                    }
                }
                Err(e) => {
                    eprintln!("Failed to collect monitor data: {e}");
                }
            }
        }
        // On loop exit, publish stopped status
        let _ = publisher.publish("monitoring.status", &serde_json::json!({"status": "stopped"})).await;
        tracing::info!("[NATS] Published monitoring.status: stopped (loop exit)");
    } else {
        // If scan is not entered, publish stopped status
        let _ = publisher.publish("monitoring.status", &serde_json::json!({"status": "stopped"})).await;
        tracing::info!("[NATS] Published monitoring.status: stopped (scan not entered)");
    }
}
async fn handle_collector_connection(stream: TcpStream, status_tx: Arc<broadcast::Sender<String>>) {
    let ws_stream = accept_async(stream).await.expect("Failed to accept websocket connection");
    let (mut ws_sender, _) = ws_stream.split();

    // Subscribe to status updates
    let mut rx = status_tx.subscribe();

    // Send initial status
    let initial_status = json!({
        "collector": "Running"
    }).to_string();
    let _ = ws_sender.send(initial_status.into()).await;

    // Forward status updates to the WebSocket client
    while let Ok(status) = rx.recv().await {
        if let Err(_) = ws_sender.send(status.into()).await {
            // Client disconnected, exit gracefully
            break;
        }
    }
}

// Function to broadcast collector status updates
pub fn broadcast_collector_status(status_tx: &Arc<broadcast::Sender<String>>, status: &str) {
    let status_json = json!({
        "collector": status
    }).to_string();
    let _ = status_tx.send(status_json); // Ignore send errors
}


#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Set up file logging
    let file_appender = rolling::never(".", "logs.txt");
    let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);

    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .with_writer(non_blocking.and(std::io::stdout))
        .with_ansi(false) // <--- enables color codes for terminal
        .compact()        // <--- makes logs more compact/readable
        .init();

    info!("Loading master key...");
    let master_key = KeyManager::load_master_key();
    info!("Master key loaded successfully");

    // === WebSocket server for collector status ===
    let (status_tx, _status_rx) = broadcast::channel::<String>(16);
    let status_tx_arc = Arc::new(status_tx);
    let status_tx_clone = status_tx_arc.clone();
    spawn(async move {
        let listener = TcpListener::bind("127.0.0.1:3032").await.expect("Failed to bind collector status WebSocket port");
        println!("✅ Collector status WebSocket running at ws://127.0.0.1:3032/ws/collector");
        loop {
            let (stream, _) = listener.accept().await.expect("Failed to accept connection");
            let status_tx_clone = status_tx_clone.clone();
            spawn(async move {
                // Only accept connections to /ws/collector
                // For simplicity, accept all connections on this port
                handle_collector_connection(stream, status_tx_clone).await;
            });
        }
    });
    // Broadcast initial status
    broadcast_collector_status(&status_tx_arc, "Running");

    // --- Service state flags ---
    let manual_running = Arc::new(AtomicBool::new(true)); // controlled by toggle endpoint
    let nats_healthy = Arc::new(AtomicBool::new(true));  // controlled by NATS health check
    let running = Arc::new(AtomicBool::new(true));       // true if both above are true

    // --- NATS health polling loop ---
    let nats_healthy_for_health = nats_healthy.clone();
    let manual_running_for_health = manual_running.clone();
    let running_for_health = running.clone();
    let status_tx_for_health = status_tx_arc.clone();
    tokio::spawn(async move {
        loop {
            let nats_ok = NatsPublisher::new(
                &CONFIG.nats_url,
                &std::fs::read_to_string(&CONFIG.c_jwt_path).unwrap_or_default(),
                &std::fs::read_to_string(&CONFIG.c_nkey_path).unwrap_or_default(),
                &CONFIG.ca_cert_path,
                &CONFIG.client_cert_path,
                &CONFIG.client_key_path,
            ).await.is_ok();
            let prev_nats = nats_healthy_for_health.load(Ordering::SeqCst);
            nats_healthy_for_health.store(nats_ok, Ordering::SeqCst);
            let manual = manual_running_for_health.load(Ordering::SeqCst);
            let should_run = nats_ok && manual;
            let was_running = running_for_health.load(Ordering::SeqCst);
            if should_run != was_running {
                running_for_health.store(should_run, Ordering::SeqCst);
                let status = if should_run {
                    "Running"
                } else if !nats_ok {
                    "Paused (NATS)"
                } else {
                    "Paused (Manual)"
                };
                broadcast_collector_status(&status_tx_for_health, status);
            }
            tokio::time::sleep(Duration::from_secs(5)).await;
        }
    });

    // Pass running to setup_nats_client
    setup_nats_client(master_key, running.clone()).await?;

    // --- Toggle endpoint uses manual_running ---
    let manual_running_for_toggle = manual_running.clone();
    let nats_healthy_for_toggle = nats_healthy.clone();
    let running_for_toggle = running.clone();
    let status_tx_for_toggle = status_tx_arc.clone();
    let app = Router::new().route(
        "/api/service/collector/toggle",
        get({
            move || {
                let manual_running = manual_running_for_toggle.clone();
                let nats_healthy = nats_healthy_for_toggle.clone();
                let running = running_for_toggle.clone();
                let status_tx = status_tx_for_toggle.clone();
                async move {
                    let new_manual = !manual_running.load(Ordering::SeqCst);
                    manual_running.store(new_manual, Ordering::SeqCst);
                    let nats_ok = nats_healthy.load(Ordering::SeqCst);
                    let should_run = nats_ok && new_manual;
                    running.store(should_run, Ordering::SeqCst);
                    let status = if should_run {
                        "Running"
                    } else if !nats_ok {
                        "Paused (NATS) "
                    } else {
                        "Paused (Manual) "
                    };
                    broadcast_collector_status(&status_tx, status);
                    Json(json!({ "status": status }))
                }
            }
        }),
    );

    tokio::spawn(async move {
        let listener = TcpListener::bind("127.0.0.1:3033").await.unwrap();
        serve(listener, app).await.unwrap();
    });

    // WebSocket handler for collector logs
async fn send_collector_logs(mut socket: warp::ws::WebSocket) {
    let mut interval = tokio::time::interval(std::time::Duration::from_secs(1));
    loop {
        interval.tick().await;
        match fetch_collector_logs().await {
            Ok(logs_json) => {
                if socket.send(warp::ws::Message::text(logs_json)).await.is_err() {
                    break;
                }
            }
            Err(_) => break,
        }
    }
}

async fn fetch_collector_logs() -> Result<String, std::io::Error> {
    let file = tokio::fs::File::open("logs.txt").await?;
    let reader = tokio::io::BufReader::new(file);
    let mut lines = reader.lines();
    let mut logs = Vec::new();
    while let Some(line) = lines.next_line().await? {
        logs.push(line);
    }
    Ok(serde_json::to_string(&logs)?)
}

    let collector_logs_route = warp::path("ws").and(warp::path("collector-logs"))
        .and(warp::ws())
        .map(|ws: warp::ws::Ws| ws.on_upgrade(send_collector_logs));

    tokio::spawn(async move {
        warp::serve(collector_logs_route)
            .run(([127, 0, 0, 1], 3034))
            .await;
    });

    signal::ctrl_c().await?;
    info!("Collector shutting down gracefully...");

    Ok(())
}
