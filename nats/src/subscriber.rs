use async_nats::ConnectOptions;
use futures::StreamExt;
use serde_json::Value;
use std::sync::Arc;
use crate::load_tls_certificates;
use nkeys::KeyPair;
use std::error::Error;
 pub struct NatsSubscriber {
    client: async_nats::Client,
}

impl NatsSubscriber {
    pub async fn new(
        nats_url: &str,
        b_jwt: &str,
        b_nkey: &str,
        ca_cert_path: &str,
        client_cert_path: &str,
        client_key_path: &str,
    ) -> Result<Self, Box<dyn Error + Send + Sync>> {
        let kp = Arc::new(KeyPair::from_seed(b_nkey)?);
        let tls_config = load_tls_certificates(ca_cert_path, client_cert_path, client_key_path)?;

        let client = async_nats::connect_with_options(
            nats_url,
            ConnectOptions::new()
                .jwt(b_jwt.to_string(), move |nonce| {
                    let kp = Arc::clone(&kp);
                    Box::pin(async move {
                        kp.sign(&nonce).map_err(|e| async_nats::AuthError::new(e.to_string()))
                    })
                })
                .require_tls(true)
                .tls_client_config((*tls_config).clone()),
        )
        .await?;

        Ok(Self { client })
    }

    /// Used for closure-based subscriptions (optional)
    pub async fn subscribe<F>(&self, subject: &str, handler: F) -> Result<(), Box<dyn Error + Send + Sync>>
    where
        F: Fn(Value) + Send + 'static,
    {
        let mut subscription = self.client.subscribe(subject.to_string()).await?;
        tokio::spawn(async move {
            while let Some(message) = subscription.next().await {
                if let Ok(json) = serde_json::from_slice::<Value>(&message.payload) {
                    handler(json);
                }
            }
        });
        Ok(())
    }

    /// NEW: Allow direct access to underlying NATS client
    pub fn client(&self) -> &async_nats::Client {
        &self.client
    }
}
