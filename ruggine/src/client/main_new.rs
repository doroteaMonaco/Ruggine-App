mod network;
mod ui;
mod config;

use clap::Parser;
use log::{info, error};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, BufWriter};
use tokio::net::TcpStream;
use std::io::{self, Write};
use config::ClientConfig;

/// Funzione helper per ottenere username dall'utente
fn get_username_from_user(provided_username: Option<String>) -> Result<String, Box<dyn std::error::Error>> {
    if let Some(username) = provided_username {
        if username.trim().is_empty() {
            return Err("Username cannot be empty".into());
        }
        Ok(username)
    } else {
        println!("📝 Enter your username (must be unique):");
        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;
        let username = input.trim().to_string();
        if username.is_empty() {
            return Err("Username cannot be empty".into());
        }
        Ok(username)
    }
}

#[derive(Parser, Debug)]
#[command(name = "ruggine-client")]
#[command(about = "A chat client application")]
struct Args {
    #[arg(long, help = "Server host address (overrides config)")]
    host: Option<String>,
    
    #[arg(short, long, help = "Server port (overrides config)")]
    port: Option<u16>,
    
    #[arg(short, long, help = "Username for connection")]
    username: Option<String>,
    
    #[arg(long, help = "Auto-connect as local host using config")]
    auto: bool,
    
    #[arg(long, help = "Connect as remote client using public host from config")]
    remote: bool,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();
    
    let args = Args::parse();
    
    // Carica la configurazione dal file .env
    let config = ClientConfig::load()?;
    
    // Determina host, porta e username
    let (host, port, username) = if args.auto {
        // Modalità automatica - host locale
        info!("🏠 Auto-connecting as local host...");
        let username = get_username_from_user(args.username)?;
        (config.default_host.clone(), config.default_port, username)
    } else if args.remote {
        // Modalità remota - usa IP pubblico
        info!("🌐 Connecting as remote client...");
        let username = get_username_from_user(args.username)?;
        (config.public_host.clone(), config.default_port, username)
    } else {
        // Modalità manuale - usa argomenti o valori predefiniti
        let host = args.host.unwrap_or(config.default_host.clone());
        let port = args.port.unwrap_or(config.default_port);
        let username = get_username_from_user(args.username)?;
        (host, port, username)
    };
    
    info!("🚀 Starting Ruggine client...");
    println!("🔗 Connecting to {}:{} as '{}'", host, port, username);
    
    // Connessione al server
    let stream = match TcpStream::connect(format!("{}:{}", host, port)).await {
        Ok(stream) => {
            println!("✅ Connected to server!");
            stream
        }
        Err(e) => {
            error!("❌ Failed to connect: {}", e);
            println!("💡 Make sure the server is running with: cargo run --bin ruggine-server");
            return Err(e.into());
        }
    };
    
    let (reader, writer) = stream.into_split();
    let mut buf_reader = BufReader::new(reader);
    let mut buf_writer = BufWriter::new(writer);
    
    // Leggi il messaggio di benvenuto
    let mut welcome_line = String::new();
    while buf_reader.read_line(&mut welcome_line).await? > 0 {
        print!("{}", welcome_line);
        if welcome_line.contains("Please register first") {
            break;
        }
        welcome_line.clear();
    }
    
    // Invia comando di registrazione
    let register_cmd = format!("/register {}\n", username);
    buf_writer.write_all(register_cmd.as_bytes()).await?;
    buf_writer.flush().await?;
    
    println!("🔐 Registering as '{}'...", username);
    
    // Spawn task per leggere messaggi dal server
    let mut buf_reader_clone = buf_reader;
    tokio::spawn(async move {
        let mut line = String::new();
        loop {
            line.clear();
            match buf_reader_clone.read_line(&mut line).await {
                Ok(0) => break, // Connection closed
                Ok(_) => {
                    print!("📨 {}", line);
                }
                Err(e) => {
                    error!("Error reading from server: {}", e);
                    break;
                }
            }
        }
    });
    
    // Loop principale per input utente
    println!("\n💬 You can now send messages! Type /help for commands, /quit to exit");
    let stdin = tokio::io::stdin();
    let mut stdin_reader = BufReader::new(stdin);
    
    loop {
        print!("💬 > ");
        io::stdout().flush()?;
        
        let mut input = String::new();
        match stdin_reader.read_line(&mut input).await {
            Ok(0) => break, // EOF
            Ok(_) => {
                let input = input.trim();
                if input == "/quit" || input == "/exit" {
                    println!("👋 Goodbye!");
                    break;
                }
                
                // Invia il messaggio al server
                buf_writer.write_all(format!("{}\n", input).as_bytes()).await?;
                buf_writer.flush().await?;
            }
            Err(e) => {
                error!("Error reading input: {}", e);
                break;
            }
        }
    }
    
    Ok(())
}
