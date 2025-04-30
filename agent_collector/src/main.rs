use tracing::{info}; // Removed `error` as it is unused
use tokio::signal;
use serde::Serialize;
use tokio::io::AsyncBufReadExt; // Import the required trait for read_line
use futures::StreamExt; // Import StreamExt to use the `next` method
use base64::{engine::general_purpose, Engine as _};
use agent_lib; // Your custom library for agent data

use shared_config::CONFIG;

mod key_utils;
use key_utils::KeyManager;

use nats::publisher::NatsPublisher;
use nats::subscriber::NatsSubscriber;
use models_database::db::{
    establish_connection, get_agent_details
};
use async_nats::Client;

#[derive(Serialize, Debug)]
struct MasterKeyPayload {
    master_key: String,
}

async fn setup_nats_client(master_key: Vec<u8>) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    
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
    };
    publisher.publish("master.key", &payload).await?;
    info!("Master key published to NATs........... ");

    // Subscribe to bridge.response topic and handle it
    let client = subscriber.client().clone();
    let subscribe_for_sacn = subscriber.client().clone(); 
    
    let pub_clone = publisher.clone(); // Clone for move into async

    let mut sub = subscriber.client().subscribe("bridge.response".to_string()).await?;
    tokio::spawn(async move {
        while let Some(msg) = sub.next().await {
            let payload = String::from_utf8_lossy(&msg.payload);
            let mut conn = establish_connection(&CONFIG.db_path);
            
            if get_agent_details(&mut conn).is_some() {
                println!("[INFO] Device details stored in database. Skipping the collecting agent data ");
                info!("Skipping the collecting agent data ");
                start_monitoring(client.clone()).await;
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
                            if let Err(e) = pub_clone.publish("agent.data", &agent_data).await {
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

                                    start_monitoring(client.clone()).await;
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
                    "disk" => {
                        info!("Scanning disk............................................");
                        match agent_lib::scan_disk(action) {
                            Ok(disk) => send_scan_response(&publisher, action, uuid_value, disk).await,
                            Err(e) => eprintln!("Failed to scan disk: {e}"),
                        }
                    }
                    "nic" => {
                        info!("Scanning nic details............................................");
                        match agent_lib::scan_nic(action) {
                            Ok(nic_data) => send_scan_response(&publisher, action, uuid_value, nic_data).await,
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

async fn send_scan_response<T: serde::Serialize + std::fmt::Debug>(publisher: &NatsPublisher,   action: &str,uuid: &str ,data: T) {
    let original_json = serde_json::json!(data);
    println!("Original JSON: {}", original_json);
    println!("Dtata: {:?}", data);
    
        let message_json = serde_json::json!({
            "uuid": uuid,
            "result": original_json,
            "action": action
        });
    
            if let Err(e) = publisher.publish(&format!("send.scan.{}", action), &message_json).await {
                eprintln!("Failed to publish agent data: {e}");
            }
       


}


async fn start_monitoring(client: Client) {
    println!("Type 'scan' to start collecting monitoring data:");

    let mut input = String::new();
    let mut stdin = tokio::io::BufReader::new(tokio::io::stdin());
    if let Err(e) = stdin.read_line(&mut input).await {
        eprintln!("Failed to read input: {e}");
        return;
    }

    if input.trim().eq_ignore_ascii_case("scan") {
        println!("Collecting the monitoring data...................");

        let mut interval = tokio::time::interval(std::time::Duration::from_secs(1));
        let mut data_queue: Vec<String> = Vec::new();

        loop {
            interval.tick().await;

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
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    tracing_subscriber::fmt().with_max_level(tracing::Level::INFO)
.init();

    info!("Loading master key...");
    let master_key = KeyManager::load_master_key();
    info!("Master key loaded successfully");

    setup_nats_client(master_key).await?;

    signal::ctrl_c().await?;
    info!("Collector shutting down gracefully...");

    Ok(())
}
