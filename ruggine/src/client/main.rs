mod network;
mod ui;

use clap::Parser;
use log::{info, error};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, BufWriter};
use tokio::net::TcpStream;
use std::io::{self, Write};

#[derive(Parser, Debug)]
#[command(name = "ruggine-client")]
#[command(about = "A chat client application")]
struct Args {
    #[arg(long, default_value = "127.0.0.1")]
    host: String,
    
    #[arg(short, long, default_value = "5000")] // Porta server, non 8080
    port: u16,
    
    #[arg(short, long)]
    username: Option<String>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();
    
    let args = Args::parse();
    
    info!("🚀 Starting Ruggine client...");
    println!("🔗 Connecting to {}:{}", args.host, args.port);
    
    // Connessione al server
    let stream = match TcpStream::connect(format!("{}:{}", args.host, args.port)).await {
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
    
    // Registrazione automatica con username
    let username = if let Some(username) = args.username {
        username
    } else {
        println!("\n📝 Enter your username:");
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        input.trim().to_string()
    };
    
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
