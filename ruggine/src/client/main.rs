mod network;
mod ui;

use clap::Parser;
use log::info;

#[derive(Parser, Debug)]
#[command(name = "ruggine-client")]
#[command(about = "A chat client application")]
struct Args {
    #[arg(short, long, default_value = "127.0.0.1")]
    host: String,
    
    #[arg(short, long, default_value = "8080")]
    port: u16,
    
    #[arg(short, long)]
    username: Option<String>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();
    
    let args = Args::parse();
    
    info!("Starting Ruggine client...");
    info!("Connecting to {}:{}", args.host, args.port);
    
    if let Some(username) = args.username {
        info!("Username: {}", username);
    }
    
    // TODO: Implement client logic here
    println!("Ruggine client started!");
    
    Ok(())
}
