use std::process::Command;
use std::path::Path;

use shared_config::CONFIG;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Path to the configuration file
    let config_path = format!("{}/nats/nats-server.conf", CONFIG.app_dir);

    // Generate the configuration file dynamically (if needed)
    if !Path::new(&config_path).exists() {
       nats::generate_nats_server_config(&config_path)?;
    }

    // Start the NATS server with the configuration file
    let mut child = Command::new("nats-server")
        .arg("-c")
        .arg(&config_path)
        .arg("-DV")
        .spawn()?;

    println!("NATS server started with configuration: {}", config_path);

    // Wait for the server process to exit (optional)
    child.wait()?;

    Ok(())
}
