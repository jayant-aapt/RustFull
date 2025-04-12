use tracing::{info, error};
use std::collections::HashMap;
use serde_json::Value;
use base64::engine::general_purpose;
use base64::Engine;
use shared_config::CONFIG;

use models_database::db::{
    establish_connection, get_agent_credential, initial_data_save, is_agent_onboarded, save_agent, ServerResponse
};


/// Submits the master key to the server for onboarding
pub async fn send_master_key_to_server(received_payload: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    
    let mut conn = establish_connection(&CONFIG.db_path);

    if is_agent_onboarded(&mut conn) {
        info!("Agent already onboarded. Skipping master key submission.");
        return Ok(());
    }

    let payload = serde_json::json!({
        "master_key": general_purpose::STANDARD.encode(received_payload),
    });

    let client = reqwest::Client::builder().danger_accept_invalid_certs(true).build()?;
    let response = client
        .post("https://192.168.100.13/api/agent/onboard/")
        .header("X-API-KEY", "1234567890abcdef1234567890abcdef")
        .json(&payload)
        .send()
        .await?;

    if response.status().is_success() {
        let parsed_response: ServerResponse = serde_json::from_str(&response.text().await?)?;
        save_agent(&mut conn, &parsed_response)?;
        info!("Master key successfully sent and agent onboarded.");
    } else {
        error!("Failed to send master key: {}", response.status());
    }

    Ok(())
}

/// Sends agent data to the server with provided token
pub async fn send_to_server(data: &Value, token: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut conn = establish_connection(&CONFIG.db_path);
    let agent_uuid = get_agent_credential(&mut conn).ok_or("No UUID found")?.uuid;

    let client = reqwest::Client::builder().danger_accept_invalid_certs(true).build()?;
    let response = client
        .post("https://192.168.100.13/api/agent/init/data/")
        .header("Authorization", format!("Bearer {}", token))
        .header("uuid", agent_uuid)
        .json(data)
        .send()
        .await?;

    if response.status().is_success() {
        let json_data: Value = serde_json::from_str(&response.text().await?)?;
        initial_data_save(&mut conn, &json_data)?;
        info!("Agent data sent to server.");
    } else {
        error!("Failed to send agent data: {}", response.status());
    }

    Ok(())
}

/// Retrieves a new access token using saved client credentials
pub async fn get_new_access_token(token_type: &str) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    let mut conn = establish_connection(&CONFIG.db_path);

    let credential = get_agent_credential(&mut conn).ok_or("No credentials found")?;

    let mut form_data = HashMap::new();
    form_data.insert("grant_type".to_string(), "client_credentials".to_string());
    form_data.insert("client_id".to_string(), credential.client_id.clone());
    form_data.insert("client_secret".to_string(), credential.client_secret.clone());

    let url = match token_type {
        "access_token" => "https://192.168.100.13/api/agent/get/jwt/access_token/",
        _ => "https://192.168.100.13/api/agent/get/jwt/",
    };

    let client = reqwest::Client::builder().danger_accept_invalid_certs(true).build()?;
    let response = client
        .post(url)
        .header("Content-Type", "application/x-www-form-urlencoded")
        .header("uuid", credential.uuid)
        .form(&form_data)
        .send()
        .await?;

    let status = response.status();
    let body = response.text().await?;

    if status.is_success() {
        info!("Successfully fetched access token.");
        Ok(body)
    } else {
        error!("Failed to get access token: {}", body);
        Err(format!("Token error: {status}").into())
    }
}
