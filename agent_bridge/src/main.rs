use tracing::{info, error};
use serde_json::Value;
use tokio::signal;

use shared_config::CONFIG; // Import CONFIG from the shared library

use nats::publisher::NatsPublisher;
use nats::subscriber::NatsSubscriber;
use futures::StreamExt;
mod server_api; 
use server_api::{send_master_key_to_server, send_to_server, get_new_access_token};

//mod config; // Add this line to include the config module

pub async fn handle_nats_operations() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    info!("Starting NATS operations...");

    // === SETUP NATS CLIENTS ===
    let publisher = NatsPublisher::new(
        &CONFIG.nats_url,
        &std::fs::read_to_string(&CONFIG.b_jwt_path)?,
        &std::fs::read_to_string(&CONFIG.b_nkey_path)?,
        &CONFIG.ca_cert_path,
        &CONFIG.bridge_cert_path,
        &CONFIG.bridge_key_path,
    )
    .await?;

    let subscriber = NatsSubscriber::new(
        &CONFIG.nats_url,
        &std::fs::read_to_string(&CONFIG.b_jwt_path)?,
        &std::fs::read_to_string(&CONFIG.b_nkey_path)?,
        &CONFIG.ca_cert_path,
        &CONFIG.bridge_cert_path,
        &CONFIG.bridge_key_path,
    )
    .await?;

    // === STEP 1: RECEIVE MASTER KEY ===
    let mut master_key_sub = subscriber.client().subscribe("master_key".to_string()).await?;
    if let Some(msg) = master_key_sub.next().await {
        let payload_str = String::from_utf8_lossy(&msg.payload);
        info!("Received master key: {}", payload_str);

        // Send the master key to the server
        if let Err(e) = send_master_key_to_server(&payload_str).await {
            error!("Failed to send master key to server: {}", e);
            return Err(e);
        }
    }

    // === STEP 2: PUBLISH ACCESS TOKEN ===
    let token_json = get_new_access_token("token").await?;
    let access_token = match serde_json::from_str::<Value>(&token_json) {
        Ok(json_val) => json_val
            .get("access_token")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        Err(e) => {
            error!("Failed to parse access token JSON: {}", e);
            return Err("Invalid access token response".into());
        }
    };

    let token_response = serde_json::json!({
        "status": "ok",
        "token": access_token,
    });

    publisher.publish("bridge.response", &token_response).await?;
    info!("Published access token to collector.");

    // === STEP 3: PROCESS AGENT DATA ===
    let mut agent_data_sub = subscriber.client().subscribe("agent.data".to_string()).await?;
    info!("Waiting for agent data...");

    while let Some(msg) = agent_data_sub.next().await {
        let payload_str = String::from_utf8_lossy(&msg.payload);
        match serde_json::from_str::<Value>(&payload_str) {
            Ok(json) => {
                // Send agent data to the server
                if let Err(e) = send_to_server(&json, &access_token).await {
                    error!("Failed to send agent data: {}", e);
                }
            }
            Err(e) => error!("Invalid JSON in agent data: {}", e),
        }
    }

    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Initialize logging
    tracing_subscriber::fmt().with_max_level(tracing::Level::INFO).init();
    info!("Bridge Application starting...");

    // Start NATS operations
    if let Err(e) = handle_nats_operations().await {
        error!("Error in NATS operations: {:?}", e);
    }

    // Wait for Ctrl+C signal to shut down gracefully
    signal::ctrl_c().await?;
    info!("Bridge shutting down gracefully...");
    Ok(())
}