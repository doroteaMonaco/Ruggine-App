mod chat_manager;
mod connection;
mod database;
mod config;

// Importiamo i moduli comuni
#[path = "../common/mod.rs"]
mod common;

#[path = "../utils/mod.rs"]
mod utils;

use clap::Parser;
use log::{info, warn, error};
use std::sync::Arc;
use std::time::Duration;
use tokio::net::TcpListener;
use tokio::sync::RwLock;
use connection::ClientConnection;
use chat_manager::ChatManager;
use database::DatabaseManager;
use config::Config;

#[derive(Parser, Debug)]
#[command(name = "ruggine-server")]
#[command(about = "A chat server application")]
struct Args {
    #[arg(long, default_value = "0.0.0.0")] // Accetta connessioni da qualsiasi IP
    host: String,
    
    #[arg(short, long, default_value = "5000")]
    port: u16,
    
    #[arg(short, long, default_value = "100")]
    max_clients: usize,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();
    
    let args = Args::parse();
    
    // Carica la configurazione dal file .env
    let config = Config::load()?;
    
    info!("Starting Ruggine server...");
    info!("Listening on {}:{}", args.host, args.port);
    info!("Maximum clients: {}", args.max_clients);
    
    // Inizializza il database usando la configurazione
    let db_manager = Arc::new(DatabaseManager::new(&config.database_url).await?);
    info!("Database initialized successfully");
    
    // Inizializza il chat manager con il database
    let chat_manager = Arc::new(ChatManager::new(Arc::clone(&db_manager)));
    let connection_count = Arc::new(RwLock::new(0usize));
    
    // Bind del listener TCP
    let listener = TcpListener::bind(format!("{}:{}", args.host, args.port)).await?;
    info!("Server successfully bound to {}:{}", args.host, args.port);
    
    // Task di monitoring delle performance (ogni 2 minuti)
    let chat_manager_monitoring = Arc::clone(&chat_manager);
    let _monitoring_task = tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(120));
        
        // Crea il file di log delle performance se non esiste
        let log_file_path = "ruggine_performance.log";
        if !std::path::Path::new(log_file_path).exists() {
            if let Err(e) = std::fs::write(log_file_path, "# Ruggine Server Performance Log\n# Timestamp, Active_Users, Groups, Total_Messages, CPU_Usage\n") {
                error!("Failed to create performance log file: {}", e);
            }
        }
        
        loop {
            interval.tick().await;
            
            // Ottieni e logga le metriche
            let (users, groups, messages) = chat_manager_monitoring.get_performance_metrics().await;
            
            // Monitoring CPU
            let mut system = sysinfo::System::new_all();
            system.refresh_cpu();
            let cpu_usage: f32 = system.cpus().iter().map(|cpu| cpu.cpu_usage()).sum::<f32>() / system.cpus().len() as f32;
            
            let timestamp = chrono::Local::now().format("%Y-%m-%d %H:%M:%S");
            
            // Log su console (come prima)
            info!("📊 Performance Metrics - Active Users: {}, Groups: {}, Total Messages: {}", 
                users, groups, messages);
            info!("🖥️ CPU Usage: {:.1}%", cpu_usage);
            
            // Log su file dedicato
            let log_entry = format!("{}, {}, {}, {}, {:.1}%\n", timestamp, users, groups, messages, cpu_usage);
            if let Err(e) = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(log_file_path)
                .and_then(|mut file| {
                    use std::io::Write;
                    file.write_all(log_entry.as_bytes())
                }) {
                error!("Failed to write to performance log: {}", e);
            } else {
                info!("� Performance data logged to {}", log_file_path);
            }
        }
    });
    
    info!("✅ Sistema di monitoring avviato (performance log ogni 2 minuti su ruggine_performance.log)");
    
    // Loop principale per accettare connessioni
    loop {
        match listener.accept().await {
            Ok((stream, addr)) => {
                info!("New connection from: {}", addr);
                
                // Controlla il limite di connessioni
                let current_connections = *connection_count.read().await;
                if current_connections >= args.max_clients {
                    warn!("Connection limit reached ({}), rejecting {}", args.max_clients, addr);
                    drop(stream);
                    continue;
                }
                
                // Incrementa il contatore
                *connection_count.write().await += 1;
                
                // Spawna un task per gestire la connessione
                let chat_manager_clone = Arc::clone(&chat_manager);
                let connection_count_clone = Arc::clone(&connection_count);
                
                tokio::spawn(async move {
                    let client_connection = ClientConnection::new(stream, addr);
                    
                    if let Err(e) = client_connection.handle(chat_manager_clone).await {
                        if e.to_string() == "DISCONNECT_REQUESTED" {
                            info!("Client {} disconnected gracefully", addr);
                        } else {
                            error!("Error handling client {}: {}", addr, e);
                        }
                    }
                    
                    // Decrementa il contatore quando la connessione termina
                    *connection_count_clone.write().await -= 1;
                    info!("Client {} disconnected", addr);
                });
            }
            Err(e) => {
                error!("Error accepting connection: {}", e);
            }
        }
    }
}