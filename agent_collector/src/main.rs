use tracing::{info, error}; // Logging
use tokio::signal;
use serde::Serialize;
use base64::{engine::general_purpose, Engine as _};
use agent_lib; // Your custom library for agent data

use shared_config::CONFIG; // Use shared CONFIG

mod key_utils;
use key_utils::KeyManager;

use nats::publisher::NatsPublisher;
use nats::subscriber::NatsSubscriber;

#[derive(Serialize)]
struct MasterKeyPayload {
    master_key: String, // Encoded master key payload
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

    // Publish the master key to the "master_key" topic
    let payload = MasterKeyPayload {
        master_key: general_purpose::STANDARD.encode(&master_key),
    };
    publisher.publish("master_key", &payload).await?;
    info!("Master key published to NATS on 'master_key'");

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

    // Subscribe to the "bridge.response" topic and handle responses
    let pub_clone = publisher.clone();
    subscriber
        .subscribe("bridge.response", move |msg| {
            let publisher = pub_clone.clone();

            tokio::spawn(async move {
                // Deserialize the message and handle it
                match serde_json::from_str::<serde_json::Value>(&msg.to_string()) {
                    Ok(json) => {
                        if json.get("status") == Some(&serde_json::Value::String("ok".to_string())) {
                            info!("Bridge response received: {:?}", json);

                            // Collect and publish agent data
                            match agent_lib::agent_data() {
                                Ok(agent_data) => {
                                    if let Err(e) = publisher
                                        .publish("agent.data", &agent_data)
                                        .await
                                    {
                                        error!("Failed to publish agent data: {e}");
                                    } else {
                                        info!("[INFO] Agent data sent to bridge");
                                    }
                                }
                                Err(e) => error!("Failed to collect agent data: {e}"),
                            }
                        }
                    }
                    Err(e) => error!("Failed to deserialize message: {e}"),
                }
            });
        })
        .await?;

    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    // Load the master key
    info!("Loading master key...");
    let master_key = KeyManager::load_master_key();
    info!("Master key loaded successfully");

    // Set up NATS client and start operations
    setup_nats_client(master_key).await?;

    // Wait for Ctrl+C signal to shut down gracefully
    signal::ctrl_c().await?;
    info!("Collector shutting down gracefully...");

    Ok(())
}
