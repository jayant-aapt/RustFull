pub mod db;
pub mod models;
pub mod schema;
pub mod initail_response;

<<<<<<< HEAD
pub use db::{save_agent, establish_connection,initial_data_save,is_agent_onboarded,get_agent_credential}; 
 
use std::fs::write;
use shared_config::CONFIG;
 
=======
pub use db::{save_agent, establish_connection, initial_data_save, is_agent_onboarded, get_agent_credential};

use std::fs::write;
use shared_config::CONFIG;

>>>>>>> 988e83801efc0fc0d06d0d1387e6971d75698051
/// Generates the `diesel.toml` file dynamically using the paths from the CONFIG struct.
pub fn generate_diesel_toml() -> Result<(), Box<dyn std::error::Error>> {
    let diesel_toml_content = format!(
        r#"# For documentation on how to configure this file,
# see https://diesel.rs/guides/configuring-diesel-cli
<<<<<<< HEAD
 
[print_schema]
file = "src/schema.rs"
custom_type_derives = ["diesel::query_builder::QueryId", "Clone"]
 
=======

[print_schema]
file = "src/schema.rs"
custom_type_derives = ["diesel::query_builder::QueryId", "Clone"]

>>>>>>> 988e83801efc0fc0d06d0d1387e6971d75698051
[migrations_directory]
dir = "{migrations_dir}"
"#,
        migrations_dir = CONFIG.db_path.replace("models_database.sqlite", "migrations")
    );
<<<<<<< HEAD
 
    // Write the generated content to the `diesel.toml` file
    write(format!("{}/models_database/diesel.toml", CONFIG.app_dir), diesel_toml_content)?;
 
    Ok(())
}
 
=======

    // Write the generated content to the `diesel.toml` file
    write(format!("{}/models_database/diesel.toml", CONFIG.app_dir), diesel_toml_content)?;

    Ok(())
}

>>>>>>> 988e83801efc0fc0d06d0d1387e6971d75698051
/// Call this function during initialization to ensure `diesel.toml` is generated.
pub fn initialize() -> Result<(), Box<dyn std::error::Error>> {
    generate_diesel_toml()?;
    Ok(())
}
<<<<<<< HEAD
=======


>>>>>>> 988e83801efc0fc0d06d0d1387e6971d75698051
