#![allow(dead_code)]

use chrono::Utc;
use log::info;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::Path;

#[allow(dead_code)]
/// Configurazione avanzata del logger per il progetto Ruggine
pub struct RuggineLogger;

impl RuggineLogger {
    /// Inizializza il logger con configurazione personalizzata
    pub fn init() -> Result<(), Box<dyn std::error::Error>> {
        env_logger::Builder::from_default_env()
            .format(|buf, record| {
                writeln!(
                    buf,
                    "[{}] [{}] [{}:{}] {}",
                    Utc::now().format("%Y-%m-%d %H:%M:%S UTC"),
                    record.level(),
                    record.file().unwrap_or("unknown"),
                    record.line().unwrap_or(0),
                    record.args()
                )
            })
            .init();

        info!("Ruggine logger initialized");
        Ok(())
    }

    /// Scrive una entry nel file di log delle performance
    pub fn log_performance_to_file(
        log_path: &Path,
        cpu_usage: f64,
        memory_usage: f64,
        connections: usize,
        messages: u64,
    ) -> Result<(), std::io::Error> {
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(log_path)?;

        let timestamp = Utc::now().format("%Y-%m-%d %H:%M:%S UTC");
        let log_entry = format!(
            "{},{:.2},{:.2},{},{}\n",
            timestamp, cpu_usage, memory_usage, connections, messages
        );

        file.write_all(log_entry.as_bytes())?;
        file.flush()?;
        
        Ok(())
    }

    /// Crea il file header per il CSV delle performance
    pub fn create_performance_log_header(log_path: &Path) -> Result<(), std::io::Error> {
        if !log_path.exists() {
            let mut file = OpenOptions::new()
                .create(true)
                .write(true)
                .open(log_path)?;

            file.write_all(b"timestamp,cpu_usage_percent,memory_usage_mb,active_connections,messages_per_minute\n")?;
            file.flush()?;
        }
        Ok(())
    }
}
