use std::env;

pub struct Config {

    #[allow(dead_code)]
    pub app_dir: String,
    #[allow(dead_code)]
    pub c_jwt_path: String,
    #[allow(dead_code)]
    pub nats_url: String,
    #[allow(dead_code)]
    pub c_nkey_path: String,
    #[allow(dead_code)]
    pub ca_cert_path: String,
    #[allow(dead_code)]
    pub client_cert_path: String, // Added field for client certificate
    #[allow(dead_code)]
    pub client_key_path: String,  // Added field for client key
}

impl Config {
    pub fn new() -> Self {
        let app_dir = env::var("APP_DIR").unwrap_or_else(|_| "D:/NewRustFull/RUSTFULL".to_string());
        Self {
           
            c_jwt_path: env::var("JWT_PATH").unwrap_or_else(|_| format!("{}/nats/nsc_creds/CollectorUser.jwt", app_dir)),
            nats_url: env::var("NATS_URL").unwrap_or_else(|_| "tls://127.0.0.1:4222".to_string()),
            c_nkey_path: env::var("NKEY_PATH").unwrap_or_else(|_| format!("{}/nats/nsc_creds/CollectorUser.nk", app_dir)),
            ca_cert_path: env::var("CA_CERT_PATH").unwrap_or_else(|_| format!("{}/nats/nats_config/certificate/ca-cert.pem", app_dir)),
            client_cert_path: env::var("CLIENT_CERT_PATH").unwrap_or_else(|_| format!("{}/nats/nats_config/certificate/collector-cert.pem", app_dir)), // Default path for client certificate
            client_key_path: env::var("CLIENT_KEY_PATH").unwrap_or_else(|_| format!("{}/nats/nats_config/certificate/collector-key.pem", app_dir)), // Default path for client key
            app_dir,
        }
    }
}

lazy_static::lazy_static! {
    pub static ref CONFIG: Config = Config::new();
}
