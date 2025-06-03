use tracing::{info, error};
use serde_json::Value;
use tokio::signal;
use regex::Regex;

use shared_config::CONFIG;

use nats::publisher::NatsPublisher;
use nats::subscriber::NatsSubscriber;
use futures::StreamExt;
use std::sync::Arc;
use tokio::sync::Mutex;
mod server_api; 
use server_api::{send_master_key_to_server, send_to_server, get_new_access_token,send_to_monitor_server,scan_data_to_server};
use models_database::db::{
    establish_connection,save_token,get_token,token_exists,delete_initial_data,
};
use crate::server_api::{send_wss_status, MONITORING_RUNNING};


use serde_json::json;
use serde::Serialize;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{SystemTime, UNIX_EPOCH, Duration};
use std::fs;
use warp::ws::{WebSocket, Message};
use tokio::fs::File;
use tokio::io::{BufReader, AsyncBufReadExt};
use tokio::time;
use futures::SinkExt;
use warp::reply::Json;
use models_database::models::{Cpu, Memory, Agent, Ip};
use tower_http::cors::{CorsLayer, Any};
use warp::Filter;
use tokio::sync::broadcast;
use std::sync::Mutex as StdMutex;
use once_cell::sync::Lazy;
use tracing_appender::rolling;
use tracing_subscriber::fmt::writer::MakeWriterExt;


//mod config; // Add this line to include the config module

pub async fn create_publisher() -> Result<NatsPublisher, Box<dyn std::error::Error + Send + Sync>> {
    let publisher = NatsPublisher::new(
        &CONFIG.nats_url,
        &std::fs::read_to_string(&CONFIG.b_jwt_path)?,
        &std::fs::read_to_string(&CONFIG.b_nkey_path)?,
        &CONFIG.ca_cert_path,
        &CONFIG.bridge_cert_path,
        &CONFIG.bridge_key_path,
    )
    .await?;
    Ok(publisher)
}

pub async fn create_subscriber() -> Result<Arc<Mutex<NatsSubscriber>>, Box<dyn std::error::Error + Send + Sync>> {
    let subscriber = NatsSubscriber::new(
        &CONFIG.nats_url,
        &std::fs::read_to_string(&CONFIG.b_jwt_path)?,
        &std::fs::read_to_string(&CONFIG.b_nkey_path)?,
        &CONFIG.ca_cert_path,
        &CONFIG.bridge_cert_path,
        &CONFIG.bridge_key_path,
    )
    .await?;
    
    // Wrap the subscriber in Arc<Mutex<>> for sharing
    Ok(Arc::new(Mutex::new(subscriber)))
}

pub async fn handle_nats_operations() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    info!("Starting NATS operations handler");

    let publisher = create_publisher().await?;
    let subscriber_master = create_subscriber().await?;
    let subscriber_agent = create_subscriber().await?;
    let subscriber_monitor = create_subscriber().await?;
    let subscriber_scan =create_subscriber().await?;


    let http_client = reqwest::Client::builder()
        .danger_accept_invalid_certs(true)
        .build()?;

    // Spawn independent handlers
    let master_key_handler = handle_master_key_operations(subscriber_master, publisher.clone(), http_client.clone());
    let agent_data_handler = handle_agent_data_operations(subscriber_agent, publisher.clone(), http_client.clone());
    let monitor_data_handler = handle_monitor_data_operations(subscriber_monitor, publisher.clone(), http_client.clone());
    let scan_data_handler = handle_scan_data_operations(subscriber_scan, publisher.clone(), http_client.clone());

    tokio::select! {
        res = master_key_handler => {
            if let Err(e) = res {
                error!("Master key handler failed: {}", e);
                return Err(Box::new(std::io::Error::new(std::io::ErrorKind::Other, e.to_string())));
            }
        }
        res = agent_data_handler => {
            if let Err(e) = res {
                error!("Agent data handler failed: {}", e);
                return Err(Box::new(std::io::Error::new(std::io::ErrorKind::Other, e.to_string())));
            }
        }
        res = monitor_data_handler => {
            if let Err(e) = res {
                error!("Monitor data handler failed: {}", e);
                return Err(Box::new(std::io::Error::new(std::io::ErrorKind::Other, e.to_string())));
            }
        }
        res = scan_data_handler => {
            if let Err(e) = res {
                error!("Monitor data handler failed: {}", e);
                return Err(Box::new(std::io::Error::new(std::io::ErrorKind::Other, e.to_string())));
            }
        }
    }

    Ok(())
}


async fn process_monitor_data(http_client: &reqwest::Client, payload: &str) -> Result<String, Box<dyn std::error::Error>> {
    info!("Processing monitor data: ");

    let mut conn = establish_connection(&CONFIG.db_path);
    
    let token = match get_token(&mut conn, "access_token") {
        Some(token) => token.token,
        None => {
            match get_new_access_token("access_token").await {
                Ok(token) => {

                    let token_json: Value = match serde_json::from_str(&token) {
                        Ok(v) => v,
                        Err(e) => {
                            error!("Failed to parse token JSON: {}", e);
                            return Err(Box::new(std::io::Error::new(std::io::ErrorKind::Other, "Failed to parse token JSON")));
                        }
                    };

                    let expires_in = token_json.get("expires_in")
                        .and_then(Value::as_i64)
                        .unwrap_or(0);
            
                    let access_token_str = token_json.get("access_token")
                        .and_then(Value::as_str)
                        .unwrap_or("");
            
                    let expiration_time = (chrono::Local::now().naive_local()
                        + chrono::Duration::seconds(expires_in))
                        .format("%Y-%m-%d %H:%M:%S")
                        .to_string();
            

                    if let Err(e) = save_token(&mut conn, access_token_str, &expiration_time, "access_token") {
                        error!("Failed to save token to DB: {}", e);
                    }
        
                    match get_token(&mut conn, "access_token") {
                        Some(token) => token.token,
                        None => {
                            error!("Refresh token also expired or not found");
                            
                            String::new()
                        }
                    }
                    
                }
                Err(e) => {
                    error!("Failed to fetch access token: {}", e);
                    String::new() 
                }
            }
        }
    };

    let result = send_to_monitor_server(payload, &token).await;

    match result {
        Ok(response_data) => Ok(response_data),
        Err(e) => {
            error!("Failed to send to monitor server: {}", e);
            return Err(Box::new(std::io::Error::new(std::io::ErrorKind::Other, e)));
        }
    }
}

// Master key operations handler
async fn handle_master_key_operations(subscriber: Arc<Mutex<NatsSubscriber>>,publisher: NatsPublisher,http_client: reqwest::Client,) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {

    info!("Master key handler started");
    let mut subscriber = subscriber.lock().await;
    let mut subscriber = subscriber.client().subscribe("master.key".to_string()).await?; 
    info!("Master key handler started");

    while let Some(msg) = subscriber.next().await {
        let received_payload = String::from_utf8_lossy(&msg.payload);
        info!("Received master key payload {}", received_payload);

        if let Err(e) = send_master_key_to_server(&received_payload).await {
            error!("Failed to send master key: {}", e);
        }
        let mut conn = establish_connection(&CONFIG.db_path);

        if !token_exists(&mut conn, "access_token") {
            info!("Token not found in the database, fetching new token...");
       
            match get_new_access_token("token").await {
                Ok(token) => {
                    let expiration_time = (chrono::Local::now().naive_local()
                    + chrono::Duration::seconds(
                        serde_json::from_str::<Value>(&token)?
                            .get("expires_in")
                            .and_then(Value::as_i64)
                            .unwrap_or(0),
                    ))
                    .format("%Y-%m-%d %H:%M:%S")
                    .to_string();

                    if let Err(e) = save_token(
                        &mut conn,
                        &serde_json::from_str::<Value>(&token)?.get("access_token").and_then(Value::as_str).unwrap_or(""),
                        &expiration_time,
                        "access_token",
                    ) {
                        error!("Failed to save token to DB: {}", e);
                    } else {
                        broadcast_token_connected();
                        // After saving the token (onboarding or refresh), also broadcast collector status
                    }


                    let response = serde_json::json!({
                        "status": "ok",
                        "token": token,
                    });

                    if let Err(e) = publisher.publish("bridge.response", &response).await {
                        error!("Failed to publish token response: {}", e);
                    }
                }
                Err(e) => error!("Failed to fetch access token: {}", e),
            }
        } else {
            info!("Token already exists in the database");
            if let Err(e) = publisher.publish("bridge.response", &serde_json::json!({"message": "Token is already exists"})).await {
                error!("Failed to publish token response: {}", e);
            }
        }
    }

    Ok(())
}

// Agent data operations handler
async fn handle_agent_data_operations(subscriber: Arc<Mutex<NatsSubscriber>>,publisher: NatsPublisher,http_client: reqwest::Client,) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    info!("Agent data handler started");

    let mut subscriber = subscriber.lock().await;
    let mut subscriber = subscriber.client().subscribe("agent.data".to_string()).await?;


    while let Some(msg) = subscriber.next().await {
        info!("Bridge: Listening for 'agent.data'...");
        let data_payload = String::from_utf8_lossy(&msg.payload);
            let mut conn = establish_connection(&CONFIG.db_path);
        

            let token = match get_token(&mut conn, "access_token") {
                Some(token) => token.token,
                None => {
                    match get_new_access_token("access_token").await {
                        Ok(token) => {

                            let token_json: Value = match serde_json::from_str(&token) {
                                Ok(v) => v,
                                Err(e) => {
                                    error!("Failed to parse token JSON: {}", e);
                                    return Err(Box::new(std::io::Error::new(std::io::ErrorKind::Other, "Failed to parse token JSON")));
                                }
                            };

                            let expires_in = token_json.get("expires_in")
                                .and_then(Value::as_i64)
                                .unwrap_or(0);
                    
                            let access_token_str = token_json.get("access_token")
                                .and_then(Value::as_str)
                                .unwrap_or("");
                    
                            let expiration_time = (chrono::Local::now().naive_local()
                                + chrono::Duration::seconds(expires_in))
                                .format("%Y-%m-%d %H:%M:%S")
                                .to_string();
                    
       
                            if let Err(e) = save_token(&mut conn, access_token_str, &expiration_time, "access_token") {
                                error!("Failed to save token to DB: {}", e);
                            } else {
                                broadcast_token_connected();
                                // After saving the token (onboarding or refresh), also broadcast collector status
                            }
                
                            match get_token(&mut conn, "access_token") {
                                Some(token) => token.token,
                                None => {
                                    error!("Refresh token also expired or not found");
                                    
                                    String::new()
                                }
                            }
                            
                        }
                        Err(e) => {
                            error!("Failed to fetch access token: {}", e);
                            String::new() 
                        }
                    }
                }
            };
            match send_to_server(&data_payload, &token).await {
                Ok(response_msg) => {
                    info!("Bridge: Server responded: {}", response_msg);

                    if let Err(e) = publisher.publish("agent.response", &response_msg).await {
                        error!("Bridge: Failed to publish response: {:?}", e);
                    } else {
                        info!("Bridge: Response sent successfully");
                    }
                }
                Err(e) => error!("Bridge: Failed to send data to server: {:?}", e),
            }
        
    }

    Ok(())
}



// Monitor data operations handler
async fn handle_monitor_data_operations(subscriber: Arc<Mutex<NatsSubscriber>>,publisher: NatsPublisher,http_client: reqwest::Client) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    info!("Monitor data handler started");
    let mut subscriber = subscriber.lock().await;
    let mut subscriber = subscriber.client().subscribe("monitor.data".to_string()).await?;
    

    while let Some(msg) = subscriber.next().await {
        let payload = String::from_utf8_lossy(&msg.payload);
        info!("Received monitor data batch ({} bytes)", payload.len());

        match process_monitor_data(&http_client, &payload).await {
            Ok(response_data) => {
                info!("Received monitor server response: {}", response_data);
                  if let Ok(json_value) = serde_json::from_str::<serde_json::Value>(&response_data) {

                    if let Some(action) = json_value.get("action").and_then(|v| v.as_str()) {
                        if action.contains("deleted") {
                            info!("Action contains 'deleted', calling delete_action");
                            let mut conn = establish_connection(&CONFIG.db_path);
                            if let Err(e) = delete_initial_data(&mut conn,&json_value) {
                                error!("Failed to delete initial data: {}", e);
                            }
                        }
                
                         else{
                            if let Err(e) = publisher.publish(&format!("scan.{}", action), &json_value).await {
                                error!("Bridge: Failed to publish response: {:?}", e);
                            } else {
                                info!("Scan response sent successfully to the collector");
                            }
                         }
                    }          
                }

                
            }
            Err(e) => error!("Failed to process monitor data batch: {}", e),
        }
    }

    Ok(())
}

pub async fn handle_scan_data_operations(
    subscriber: Arc<Mutex<NatsSubscriber>>,
    _publisher: NatsPublisher,
    _http_client: reqwest::Client,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut subscribers = subscriber.lock().await;
    let mut new_subscriber = subscribers.client().subscribe("send.scan.>".to_string()).await?;
    info!("Listening for scan data...");

    while let Some(response_msg) = new_subscriber.next().await {
        let response_payload = String::from_utf8_lossy(&response_msg.payload);
        info!("Received raw response: {}", response_payload);

        let json: Value = match serde_json::from_str(&response_payload) {
            Ok(j) => j,
            Err(e) => {
                error!("Failed to parse JSON: {}", e);
                continue;
            }
        };


        if let (Some(action), Some(uuid), Some(result)) = (
            json.get("action").and_then(|v| v.as_str()),
            json.get("uuid").and_then(|v| v.as_str()),
            json.get("result"),
        ) {
            info!("Action: {}, UUID: {}, Result: {}", action, uuid, result);


            if let Err(e) = scan_data_to_server(result, uuid, action).await {
                error!("Failed to send scan data to server: {}", e);
            }
        } else {
            error!("Missing required fields in received message.");
        }
    }

    Ok(())
}
// Heartbeat WebSocket
/// Simple function to check if bridge is running
pub async fn check_bridge_status(running: Arc<AtomicBool>) -> String {
    if (!running.load(Ordering::SeqCst)) {
        return "Paused".to_string();
    }
    match create_publisher().await {
        Ok(_) => {
            let timestamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs();
            if let Err(e) = fs::write("bridge_heartbeat.txt", timestamp.to_string()) {
                error!("Failed to write heartbeat: {}", e);
                return "Stopped".to_string();
            }
            "Running".to_string()
        },
        Err(e) => {
            error!("NATS connection failed: {}", e);
            "Disconnected".to_string()
        }
    }
}

// Bridge Status WebSocket
async fn send_bridge_status(mut socket: WebSocket, running: Arc<AtomicBool>) {
    let mut interval = tokio::time::interval(Duration::from_secs(5));
    loop {
        interval.tick().await;
        
        // Check if bridge is running
        let status = check_bridge_status(running.clone()).await;
        
        let status_json = json!({ "bridge": status });
        if socket.send(Message::text(status_json.to_string())).await.is_err() {
            info!("Frontend disconnected from Bridge status check");
            break;
        }
    }
}
async fn send_logs(mut socket: warp::ws::WebSocket) {
    let mut interval = time::interval(Duration::from_secs(1)); // Send logs every second

    loop {
        interval.tick().await;

        match fetch_logs().await {
            Ok(logs_json) => {
                if socket.send(Message::text(logs_json)).await.is_err() {
                    error!("❌ Frontend disconnected from logs WebSocket");
                    break;
                }
            }
            Err(e) => {
                let error_message = json!(["Failed to read logs", e.to_string()]);
                let _ = socket.send(Message::text(error_message.to_string())).await;
                break;
            }
        }
    }
}


async fn fetch_logs() -> Result<String, std::io::Error> {
    let file = File::open("agent_bridge/logs.txt").await?;
    let reader = BufReader::new(file);
    let mut lines = reader.lines();
    let mut logs = Vec::new();
    // Regex to match ANSI escape codes
    let ansi_regex = Regex::new(r"\x1b\[[0-9;]*[a-zA-Z]").unwrap();

    while let Some(line) = lines.next_line().await? {
        // Strip ANSI escape codes
        let clean_line = ansi_regex.replace_all(&line, "").to_string();
        logs.push(clean_line);
    }

    // Return all logs, no line limit
    let logs_json = serde_json::to_string(&logs)?;
    Ok(logs_json)
}

// Pagination handler for logs
#[derive(serde::Deserialize)]
struct LogPage {
    page: usize // 0-based page index
}

async fn get_logs_handler(query: LogPage) -> Result<impl warp::Reply, warp::Rejection> {
    use tokio::io::{AsyncBufReadExt, BufReader};
    use tokio::fs::File;
    let file = File::open("agent_bridge/logs.txt").await.map_err(|_| warp::reject())?;
    let reader = BufReader::new(file);
    let mut lines = reader.lines();
    let mut all_logs = Vec::new();
    let ansi_regex = Regex::new(r"\x1b\[[0-9;]*[a-zA-Z]").unwrap();
    while let Some(line) = lines.next_line().await.map_err(|_| warp::reject())? {
        let clean_line = ansi_regex.replace_all(&line, "").to_string();
        all_logs.push(clean_line);
    }
    Ok(warp::reply::json(&all_logs))
}


// ✅ WSS server with multiple routes
async fn run_ws_servers(running: Arc<AtomicBool>) {
    let running_for_bridge_ws = running.clone();
    let running_for_toggle = running.clone();
    let wss_route = warp::path!("ws" / "wss")
        .and(warp::ws())
        .map(|ws: warp::ws::Ws| ws.on_upgrade(send_wss_status));

    let https_route = warp::path!("ws" / "https")
        .and(warp::ws())
        .map(|ws: warp::ws::Ws| ws.on_upgrade(send_https_status));

    let bridge_route = warp::path!("ws" / "bridge")
        .and(warp::ws())
        .and(warp::any().map(move || running_for_bridge_ws.clone()))
        .map(|ws: warp::ws::Ws, running: Arc<AtomicBool>| ws.on_upgrade(move |socket| send_bridge_status(socket, running)));

    let agent_route = warp::path!("ws" / "agent")
        .and(warp::ws())
        .map(|ws: warp::ws::Ws| ws.on_upgrade(send_agent_connection_status));
   
    let system_info_route = warp::path!("api" / "system-info")
        .and(warp::get())
        .and_then(get_system_info_handler);

    let logs_route = warp::path!("ws" / "logs")
        .and(warp::ws())
        .map(|ws: warp::ws::Ws| ws.on_upgrade(send_logs));

    let logs_api_route = warp::path!("api" / "logs")
        .and(warp::get())
        .and(warp::query::<LogPage>())
        .and_then(get_logs_handler);

    // Updated health check handler
    fn health_check_handler() -> impl warp::Reply {
        "OK"
    }

    let health_route = warp::path!("health")
        .and(warp::get())
        .map(health_check_handler);

    // Add these routes:
    let running_for_toggle_1 = running.clone();
    let running_for_toggle_2 = running.clone();
    let toggle_bridge_route = warp::path!("api" / "service" / "bridge" / "toggle")
        .and(warp::post())
        .and(warp::any().map(move || running_for_toggle_1.clone()))
        .and_then(toggle_bridge_handler);

    let restart_bridge_route = warp::path!("api" / "service" / "bridge" / "restart")
        .and(warp::post())
        .and(warp::any().map(move || running_for_toggle_2.clone()))
        .and_then(restart_bridge_handler);

        println!("✅ WSS running at ws://127.0.0.1:3030/ws/wss");
        println!("✅ HTTPS status check running at ws://127.0.0.1:3030/ws/https");
        println!("✅ Bridge status running at ws://127.0.0.1:3030/ws/bridge");
        println!("✅ Agent connection status running at ws://127.0.0.1:3030/ws/agent");
        println!("✅ System info API running at http://127.0.0.1:3030/api/system_info");

    let cors = CorsLayer::new().allow_origin(Any);

    let app = warp::serve(
        wss_route
            // .or(nats_route)
            // .or(combined_status_route)
            .or(https_route)
            .or(bridge_route)
            .or(agent_route)
            // .or(status_route)
            .or(system_info_route)
            .or(logs_route) // Add the logs route here
            .or(logs_api_route) // <-- add here
            .or(health_route)
            .or(toggle_bridge_route)
            .or(restart_bridge_route)
    );

    app.run(([127, 0, 0, 1], 3030)).await;
}

#[derive(Serialize, Debug)] // Add Debug here
struct SystemInfo {
    cpu: String,
    memory: String,
    os: String,
    ip_address: String,
    hostname: String,
}

async fn get_system_info_handler() -> Result<Json, warp::Rejection> {
    info!("Fetching system information...");

    let mut conn = establish_connection(&CONFIG.db_path);

    let cpu = Cpu::first(&mut conn)
        .map_or("Unknown".to_string(), |c| format!("{} @ {} ", c.model, c.speed));
    let memory = Memory::first(&mut conn)
        .map_or("Unknown".to_string(), |m| format!("{} ", m.size));
    let os = Agent::first(&mut conn)
        .map_or("Unknown".to_string(), |a| a.os);
    let ip_address = Ip::first(&mut conn)
        .map_or("Unknown".to_string(), |ip| ip.address);
    let hostname = Agent::first(&mut conn)
        .map_or("Unknown".to_string(), |a| a.hostname); // Fetch hostname // Fetch IP address
   
    let system_info = SystemInfo { cpu, memory, os, ip_address, hostname };

    info!("System information fetched: {:?}", system_info);

    Ok(warp::reply::json(&system_info))
}

// Handler to toggle (start/stop) the bridge service
async fn toggle_bridge_handler(running: Arc<AtomicBool>) -> Result<impl warp::Reply, warp::Rejection> {
    let was_running = running.load(Ordering::SeqCst);
    let new_state = !was_running;
    running.store(new_state, Ordering::SeqCst);
    let status = if new_state { "started" } else { "stopped" };
    info!("Bridge service {} via toggle endpoint", status);
    Ok(warp::reply::json(&serde_json::json!({"status": status})))
}

// Function to broadcast bridge status updates (for WebSocket, similar to collector)
fn broadcast_bridge_status(status_tx: &Arc<tokio::sync::broadcast::Sender<String>>, status: &str) {
    let status_json = serde_json::json!({ "bridge": status }).to_string();
    let _ = status_tx.send(status_json);
}

// Handler to restart the bridge service
async fn restart_bridge_handler(running: Arc<AtomicBool>) -> Result<impl warp::Reply, warp::Rejection> {
    info!("Restarting bridge service via restart endpoint");
    // Pause the bridge
    running.store(false, Ordering::SeqCst);
    info!("Bridge service paused for restart");
    tokio::time::sleep(std::time::Duration::from_secs(2)).await; // Give time for shutdown
    // Resume the bridge
    running.store(true, Ordering::SeqCst);
    info!("Bridge service resumed after restart");
    Ok(warp::reply::json(&serde_json::json!({"status": "restarted"})))
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Ensure logs.txt exists and is valid UTF-8, recreate if needed
    let log_path = std::path::Path::new("agent_bridge/logs.txt");
    if !log_path.exists() {
        match std::fs::File::create(log_path) {
            Ok(_) => info!("logs.txt did not exist, created new file at {:?}", log_path),
            Err(e) => error!("Failed to create logs.txt at {:?}: {}", log_path, e),
        }
    } else {
        // Try to read as UTF-8, recreate if invalid
        match std::fs::read_to_string(log_path) {
            Ok(_) => {},
            Err(e) => {
                error!("logs.txt exists but is not valid UTF-8 or unreadable: {}. Recreating...", e);
                match std::fs::File::create(log_path) {
                    Ok(_) => info!("logs.txt recreated at {:?}", log_path),
                    Err(e) => error!("Failed to recreate logs.txt at {:?}: {}", log_path, e),
                }
            }
        }
    }

    let file_appender = rolling::never("agent_bridge", "logs.txt");
    let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);

    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .with_writer(non_blocking.and(std::io::stdout))
        .init();

    info!("Bridge Application starting...");

    // --- On startup, check if a valid access token exists and broadcast status ---
    {
        let mut conn = establish_connection(&CONFIG.db_path);
        if let Some(token) = get_token(&mut conn, "access_token") {
            if !token.token.is_empty() {
                broadcast_token_connected();
            }
        }
    }

    // Start the WebSocket server for frontend connections
    let running = Arc::new(AtomicBool::new(true));
    tokio::spawn(run_ws_servers(running.clone()));

    // --- NATS health polling loop ---
    let running_for_health = running.clone();
    tokio::spawn(async move {
        loop {
            // Try to create a NATS publisher as a health check
            let nats_ok = create_publisher().await.is_ok();
            let was_running = running_for_health.load(Ordering::SeqCst);
            if nats_ok {
                if !was_running {
                    info!("NATS healthy, resuming bridge operations");
                    running_for_health.store(true, Ordering::SeqCst);
                }
            } else {
                if was_running {
                    error!("NATS unhealthy, pausing bridge operations");
                    running_for_health.store(false, Ordering::SeqCst);
                }
            }
            tokio::time::sleep(Duration::from_secs(5)).await;
        }
    });

    // Set up NATS subscriber for monitoring.status
    let subscriber = create_subscriber().await?;
    let subscriber = subscriber.lock().await;
    let mut sub = subscriber.client().subscribe("monitoring.status").await?;
    tokio::spawn(async move {
        use std::sync::atomic::Ordering;
        use crate::server_api::MONITORING_RUNNING;
        use crate::server_api::set_token_available;
        while let Some(msg) = sub.next().await {
            tracing::info!("[NATS] Received monitoring.status message: {:?}", msg.payload);
            if let Ok(json) = serde_json::from_slice::<serde_json::Value>(&msg.payload) {
                if let Some(status) = json.get("status").and_then(|v| v.as_str()) {
                    tracing::info!("[NATS] monitoring.status value: {}", status);
                    match status {
                        "running" => {
                            MONITORING_RUNNING.store(true, Ordering::SeqCst);
                            set_token_available(true); // Ensure token is available for WSS
                            tracing::info!("[NATS] MONITORING_RUNNING set to true, token available");
                        },
                        "stopped" => {
                            MONITORING_RUNNING.store(false, Ordering::SeqCst);
                            set_token_available(false);
                            tracing::info!("[NATS] MONITORING_RUNNING set to false, token unavailable");
                        },
                        _ => {}
                    }
                }
            } else {
                tracing::warn!("[NATS] Failed to parse monitoring.status payload as JSON");
            }
        }
    });

    // Main NATS operations loop, only runs when running is true
    loop {
        if running.load(Ordering::SeqCst) {
            if let Err(e) = handle_nats_operations().await {
                error!("Error in NATS operations: {:?}", e);
            }
        } else {
            // Paused, wait until running is true again
            tokio::time::sleep(Duration::from_secs(2)).await;
        }
    }

    //signal::ctrl_c().await?;
    //info!("Bridge shutting down gracefully...");
    //Ok(())
}

// Static broadcast channels for agent and https status
static AGENT_STATUS_CHANNEL: once_cell::sync::Lazy<broadcast::Sender<String>> = once_cell::sync::Lazy::new(|| {
    let (tx, _) = broadcast::channel(8);
    tx
});
static HTTPS_STATUS_CHANNEL: once_cell::sync::Lazy<broadcast::Sender<String>> = once_cell::sync::Lazy::new(|| {
    let (tx, _) = broadcast::channel(8);
    tx
});


static LAST_AGENT_STATUS: Lazy<StdMutex<String>> = Lazy::new(|| StdMutex::new("Disconnected".to_string()));
static LAST_HTTPS_STATUS: Lazy<StdMutex<String>> = Lazy::new(|| StdMutex::new("Disconnected".to_string()));

fn broadcast_token_connected() {
    let _ = AGENT_STATUS_CHANNEL.send("Connected".to_string());
    let _ = HTTPS_STATUS_CHANNEL.send("Connected".to_string());
    *LAST_AGENT_STATUS.lock().unwrap() = "Connected".to_string();
    *LAST_HTTPS_STATUS.lock().unwrap() = "Connected".to_string();
}

// Agent connection status WebSocket
async fn send_agent_connection_status(mut socket: WebSocket) {
    let mut rx = AGENT_STATUS_CHANNEL.subscribe();
    // Send latest status
    let last = { LAST_AGENT_STATUS.lock().unwrap().clone() };
    let _ = socket.send(Message::text(json!({"agent": last}).to_string())).await;
    loop {
        match rx.recv().await {
            Ok(status) => {
                *LAST_AGENT_STATUS.lock().unwrap() = status.clone();
                let msg = json!({"agent": status}).to_string();
                if socket.send(Message::text(msg)).await.is_err() {
                    break;
                }
            }
            Err(_) => break,
        }
    }
}
// HTTPS status WebSocket
async fn send_https_status(mut socket: WebSocket) {
    let mut rx = HTTPS_STATUS_CHANNEL.subscribe();
    // Send latest status
    let last = { LAST_HTTPS_STATUS.lock().unwrap().clone() };
    let _ = socket.send(Message::text(json!({"https": last}).to_string())).await;
    loop {
        match rx.recv().await {
            Ok(status) => {
                *LAST_HTTPS_STATUS.lock().unwrap() = status.clone();
                let msg = json!({"https": status}).to_string();
                if socket.send(Message::text(msg)).await.is_err() {
                    break;
                }
            }
            Err(_) => break,
        }
    }
}