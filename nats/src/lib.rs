use rustls::{Certificate, ClientConfig, PrivateKey, RootCertStore};
use rustls_pemfile::{certs, pkcs8_private_keys};
use std::fs::File;
use std::fs::write;
use std::io::BufReader;
use std::sync::Arc;
use std::error::Error;
use shared_config::CONFIG;

pub mod publisher;
pub mod subscriber;

pub fn load_tls_certificates(
    ca_cert_path: &str,
    client_cert_path: &str,
    client_key_path: &str,
) -> Result<Arc<ClientConfig>, Box<dyn Error + Send + Sync>> {
    let mut root_store = RootCertStore::empty();
    let mut ca_cert_file = BufReader::new(File::open(ca_cert_path)?);
    for cert in certs(&mut ca_cert_file)? {
        root_store.add(&Certificate(cert))?;
    }

    let client_certs: Vec<Certificate> = certs(&mut BufReader::new(File::open(client_cert_path)?))?
        .into_iter()
        .map(Certificate)
        .collect();
    let client_keys: Vec<PrivateKey> = pkcs8_private_keys(&mut BufReader::new(File::open(client_key_path)?))?
        .into_iter()
        .map(PrivateKey)
        .collect();

    if client_keys.is_empty() {
        return Err("No valid private keys found".into());
    }

    Ok(Arc::new(
        ClientConfig::builder()
            .with_safe_defaults()
            .with_root_certificates(root_store)
            .with_client_auth_cert(client_certs, client_keys[0].clone())?,
     ))
}

pub fn generate_nats_server_config(output_path: &str) -> Result<(), Box<dyn std::error::Error>> {
    let config_content = format!(
        r#"
listen: "0.0.0.0:4222"

operator: "{app_dir}/nats/nsc_creds/NATSOperator.jwt"

resolver {{
  type: full
  dir: "{app_dir}/nats/nsc_creds/jwt_store"
  interval: "30s"  
}}

system_account: "AD7OQRBMKJSXWTMMZSDPTGNCV3ZAJPHWS33TRBTM273AHRW6S67UDLW3"

jetstream {{
  store_dir: "{app_dir}/nats/nats_config/jetstream"
  max_mem: 1G
  max_file: 10G
}}

tls {{       
  cert_file: "{bridge_cert_path}"
  key_file: "{bridge_key_path}"
  ca_file: "{ca_cert_path}"
  verify: true
  timeout: 2
}}

authorization {{
  timeout: 20
}}
"#,
        app_dir = CONFIG.app_dir,
        ca_cert_path = CONFIG.ca_cert_path,
        bridge_cert_path = CONFIG.bridge_cert_path,
        bridge_key_path = CONFIG.bridge_key_path,
    );

    // Write the generated configuration to the specified output path
    write(output_path, config_content)?;

    Ok(())
}