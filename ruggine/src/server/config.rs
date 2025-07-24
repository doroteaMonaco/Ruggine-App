use std::env;
use std::path::Path;

#[derive(Debug, Clone)]
pub struct Config {
    pub database_url: String,
    pub sqlite_synchronous: String,
    pub sqlite_journal_mode: String,
    pub sqlite_cache_size: String,
    pub cleanup_days: u32,
    pub performance_metrics_retention_days: u32,
    pub backup_enabled: bool,
    pub backup_interval_hours: u32,
}

impl Config {
    pub fn load() -> Result<Self, Box<dyn std::error::Error>> {
        // Carica il file .env se esiste
        if Path::new(".env").exists() {
            dotenvy::dotenv().ok();
        }

        let config = Config {
            database_url: env::var("DATABASE_URL")
                .unwrap_or_else(|_| "sqlite:data/ruggine.db".to_string()),
            sqlite_synchronous: env::var("SQLITE_SYNCHRONOUS")
                .unwrap_or_else(|_| "NORMAL".to_string()),
            sqlite_journal_mode: env::var("SQLITE_JOURNAL_MODE")
                .unwrap_or_else(|_| "WAL".to_string()),
            sqlite_cache_size: env::var("SQLITE_CACHE_SIZE")
                .unwrap_or_else(|_| "10000".to_string()),
            cleanup_days: env::var("CLEANUP_DAYS")
                .unwrap_or_else(|_| "30".to_string())
                .parse()
                .unwrap_or(30),
            performance_metrics_retention_days: env::var("PERFORMANCE_METRICS_RETENTION_DAYS")
                .unwrap_or_else(|_| "7".to_string())
                .parse()
                .unwrap_or(7),
            backup_enabled: env::var("BACKUP_ENABLED")
                .unwrap_or_else(|_| "true".to_string())
                .parse()
                .unwrap_or(true),
            backup_interval_hours: env::var("BACKUP_INTERVAL_HOURS")
                .unwrap_or_else(|_| "24".to_string())
                .parse()
                .unwrap_or(24),
        };

        log::info!("Configuration loaded:");
        log::info!("  Database URL: {}", config.database_url);
        log::info!("  SQLite Synchronous: {}", config.sqlite_synchronous);
        log::info!("  SQLite Journal Mode: {}", config.sqlite_journal_mode);
        log::info!("  SQLite Cache Size: {}", config.sqlite_cache_size);
        log::info!("  Cleanup Days: {}", config.cleanup_days);
        log::info!("  Performance Metrics Retention: {} days", config.performance_metrics_retention_days);
        log::info!("  Backup Enabled: {}", config.backup_enabled);
        log::info!("  Backup Interval: {} hours", config.backup_interval_hours);

        Ok(config)
    }
}
