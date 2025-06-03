use std::env;

pub struct Config {
    pub app_dir: String,
    pub b_jwt_path: String,
    pub b_nkey_path: String,
    pub nats_url: String,
    pub c_jwt_path: String,
    pub c_nkey_path: String,
    pub jwt_private_key_path: String,
    pub jwt_public_key_path: String,
    pub ca_cert_path: String,
    pub bridge_cert_path: String,
    pub bridge_key_path: String,
    pub client_cert_path: String,
    pub client_key_path: String,
    pub central_server_url: String,
    pub db_path: String,
    pub web_socket_url: String,
}

impl Config {
    pub fn new() -> Self {
        let app_dir = env::var("APP_DIR").unwrap_or_else(|_| "C:/Users/Administrator/Downloads/RustFull".to_string());
        let config = Self {
            //bridge paths:
            b_jwt_path: env::var("JWT_PATH").unwrap_or_else(|_| format!("{}/nats/nsc_creds/BridgeUser.jwt", app_dir)),
            b_nkey_path: env::var("NKEY_PATH").unwrap_or_else(|_| format!("{}/nats/nsc_creds/BridgeUser.nk", app_dir)),
            bridge_cert_path: env::var("BRIDGE_CERT_PATH").unwrap_or_else(|_| format!("{}/nats/nats_config/certificate/bridge-cert.pem", app_dir)),
            bridge_key_path: env::var("BRIDGE_KEY_PATH").unwrap_or_else(|_| format!("{}/nats/nats_config/certificate/bridge-key.pem", app_dir)),
            
            //collector paths:
            c_jwt_path: env::var("JWT_PATH").unwrap_or_else(|_| format!("{}/nats/nsc_creds/CollectorUser.jwt", app_dir)),
            c_nkey_path: env::var("NKEY_PATH").unwrap_or_else(|_| format!("{}/nats/nsc_creds/CollectorUser.nk", app_dir)),
            client_cert_path: env::var("CLIENT_CERT_PATH").unwrap_or_else(|_| format!("{}/nats/nats_config/certificate/collector-cert.pem", app_dir)),
            client_key_path: env::var("CLIENT_KEY_PATH").unwrap_or_else(|_| format!("{}/nats/nats_config/certificate/collector-key.pem", app_dir)),
            
            //common paths:
            jwt_private_key_path: env::var("JWT_PRIVATE_KEY_PATH").unwrap_or_else(|_| format!("{}/nats/jwt_keys/private.pem", app_dir)),
            jwt_public_key_path: env::var("JWT_PUBLIC_KEY_PATH").unwrap_or_else(|_| format!("{}/nats/jwt_keys/public.pem", app_dir)),
            nats_url: env::var("NATS_URL").unwrap_or_else(|_| "tls://127.0.0.1:4222".to_string()),
            ca_cert_path: env::var("CA_CERT_PATH").unwrap_or_else(|_| format!("{}/nats/nats_config/certificate/ca-cert.pem", app_dir)),
            

            db_path: env::var("DB_PATH").unwrap_or_else(|_| format!("{}/models_database/models_database.sqlite", app_dir)),
            

            central_server_url :env::var("CENTRAL_SERVER_URL").unwrap_or_else(|_| "https://192.168.100.13".to_string()),
            web_socket_url:env::var("WEB_SOCKET_URL").unwrap_or_else(|_| "wss://192.168.100.13".to_string()),

            app_dir,
        };

        config
    }
}

lazy_static::lazy_static! {
    pub static ref CONFIG: Config = Config::new();
}
