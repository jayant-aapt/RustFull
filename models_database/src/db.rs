use diesel::prelude::*;
use diesel::sqlite::SqliteConnection;
use std::fs;
use std::path::Path;
use crate::models::AgentCredential;
use crate::schema::agent_credential::dsl::*;
use serde::{Deserialize, Serialize};
use serde_json::Value; // Added import for JSON handling
use crate::initail_response::store_json_data; // Importing the function to store JSON data
#[derive(Debug, Deserialize, Serialize)]
pub struct ServerResponse {
    pub uuid: String,
    pub client_id: String,
    pub client_secret: String,
    pub master_key: String,
}

pub fn establish_connection(db_path: &str) -> SqliteConnection {
    let db_dir = Path::new(db_path).parent().unwrap_or_else(|| Path::new("."));

    // Create directory if it doesn't exist
    if !db_dir.exists() {
        println!("Creating directory: {:?}", db_dir);
        fs::create_dir_all(db_dir).expect("Failed to create database directory");
    }

    // Create database file if it doesn't exist
    if !Path::new(db_path).exists() {
        println!("Creating database at: {}", db_path);
        fs::File::create(db_path).expect("Failed to create database file");
    }

    // Connect to the database
    let conn = SqliteConnection::establish(db_path)
        .unwrap_or_else(|_| panic!("Error connecting to {}", db_path));

    conn
}

// Function to save the response into the database
pub fn save_agent(conn: &mut SqliteConnection, response: &ServerResponse) -> Result<(), diesel::result::Error> {
    let new_agent = AgentCredential {
        id: None, // Changed to None for auto-increment
        uuid: response.uuid.clone(),
        client_id: response.client_id.clone(),
        client_secret: response.client_secret.clone(),
        master_key: response.master_key.clone(),
    };

    diesel::insert_into(agent_credential)
        .values(&new_agent)
        .execute(conn)?;

    Ok(())
}

pub fn is_agent_onboarded(conn: &mut SqliteConnection) -> bool {
    use crate::schema::agent_credential::dsl::*;
    use diesel::prelude::*;

    match agent_credential
        .select(id)
        .first::<Option<i32>>(conn)
    {
        Ok(_) => true, 
        Err(diesel::result::Error::NotFound) => false, 
        Err(_) => {
            println!("[ERROR] Failed to check onboarding status from the database.");
            false
        }
    }
}

pub fn get_agent_credential(conn: &mut SqliteConnection) -> Option<AgentCredential> {
    agent_credential
        .limit(1)
        .first::<AgentCredential>(conn)
        .ok()
} 

pub fn initial_data_save(conn: &mut SqliteConnection, json_data: &Value) -> Result<(), diesel::result::Error> {
    store_json_data(conn, json_data)?; // Fixed function call
    Ok(())
}