use dotenvy::dotenv;
use log::info;
use std::env;

#[derive(Debug, Clone)]
pub struct ClientConfig {
    pub default_host: String,
    pub default_port: u16,
    pub public_host: String,
}

impl ClientConfig {
    pub fn load() -> Result<Self, Box<dyn std::error::Error>> {
        // Carica il file .env
        dotenv().ok();
        
        let default_host = env::var("CLIENT_DEFAULT_HOST")
            .unwrap_or_else(|_| "127.0.0.1".to_string());
        
        let default_port = env::var("CLIENT_DEFAULT_PORT")
            .unwrap_or_else(|_| "5000".to_string())
            .parse::<u16>()
            .unwrap_or(5000);
        
        let public_host = env::var("CLIENT_PUBLIC_HOST")
            .unwrap_or_else(|_| "95.234.28.229".to_string());
        
        let config = ClientConfig {
            default_host,
            default_port,
            public_host,
        };
        
        info!("Client configuration loaded:");
        info!("  Default Host (localhost): {}", config.default_host);
        info!("  Default Port: {}", config.default_port);
        info!("  Public Host (for remote clients): {}", config.public_host);
        
        Ok(config)
    }
}
