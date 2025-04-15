use models_database::initialize;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize the library (e.g., generate diesel.toml)
    initialize()?;
    println!("models_database initialized successfully.");
    Ok(())
}
