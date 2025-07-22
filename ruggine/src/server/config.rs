use std::env;
use anyhow::Result;

#[derive(Debug, Clone)]
pub struct ServerConfig {
    pub database_url: String,
    pub cleanup_days: i64,
    pub performance_metrics_retention_days: i64,
    pub backup_enabled: bool,
    pub backup_interval_hours: u64,
}

impl ServerConfig {
    pub fn from_env() -> Result<Self> {
        // Carica il file .env se esiste
        dotenv::dotenv().ok();
        
        let database_url = env::var("DATABASE_URL")
            .unwrap_or_else(|_| "sqlite:ruggine.db".to_string());
            
        let cleanup_days = env::var("CLEANUP_DAYS")
            .unwrap_or_else(|_| "30".to_string())
            .parse::<i64>()
            .unwrap_or(30);
            
        let performance_metrics_retention_days = env::var("PERFORMANCE_METRICS_RETENTION_DAYS")
            .unwrap_or_else(|_| "7".to_string())
            .parse::<i64>()
            .unwrap_or(7);
            
        let backup_enabled = env::var("BACKUP_ENABLED")
            .unwrap_or_else(|_| "true".to_string())
            .parse::<bool>()
            .unwrap_or(true);
            
        let backup_interval_hours = env::var("BACKUP_INTERVAL_HOURS")
            .unwrap_or_else(|_| "24".to_string())
            .parse::<u64>()
            .unwrap_or(24);

        Ok(ServerConfig {
            database_url,
            cleanup_days,
            performance_metrics_retention_days,
            backup_enabled,
            backup_interval_hours,
        })
    }
}
