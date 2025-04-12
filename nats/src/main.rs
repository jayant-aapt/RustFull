use std::process::Command;
use std::path::Path;
use nats::generate_nats_server_config;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Path to the configuration file
    let config_path = "d:/NewRustFull/RustFull/nats/nats-server.conf";

    // Generate the configuration file dynamically (if needed)
    if !Path::new(config_path).exists() {
        generate_nats_server_config(config_path)?;
    }

    // Start the NATS server with the configuration file
    let mut child = Command::new("nats-server")
        .arg("-c")
        .arg(config_path)
        .arg("-DV")
        .spawn()?;

    println!("NATS server started with configuration: {}", config_path);

    // Wait for the server process to exit (optional)
    child.wait()?;

    Ok(())
}
