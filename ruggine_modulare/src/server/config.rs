use std::env;

#[derive(Debug, Clone)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
    pub database_url: String,
}

impl ServerConfig {
    pub fn from_env() -> Self {
        dotenvy::dotenv().ok();
        Self {
            host: env::var("SERVER_HOST").unwrap_or_else(|_| "127.0.0.1".to_string()),
            port: env::var("SERVER_PORT").ok().and_then(|p| p.parse().ok()).unwrap_or(5000),
            database_url: env::var("DATABASE_URL").unwrap_or_else(|_| "sqlite:data/ruggine_modulare.db".to_string()),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ClientConfig {
    pub default_host: String,
    pub public_host: String,
}

impl ClientConfig {
    pub fn from_env() -> Self {
        dotenvy::dotenv().ok();
        Self {
            default_host: env::var("CLIENT_DEFAULT_HOST").unwrap_or_else(|_| "127.0.0.1".to_string()),
            public_host: env::var("CLIENT_PUBLIC_HOST").unwrap_or_else(|_| "remote.example.com".to_string()),
        }
    }
}
