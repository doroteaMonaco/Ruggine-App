// Module declarations
mod client;
mod server;
mod common;
mod utils;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "ruggine")]
#[command(about = "A Rust-based chat application")]
struct Args {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Start the server
    Server {
        #[arg(short, long, default_value = "127.0.0.1")]
        host: String,
        #[arg(short, long, default_value = "8080")]
        port: u16,
    },
    /// Start the client
    Client {
        #[arg(short, long, default_value = "127.0.0.1")]
        host: String,
        #[arg(short, long, default_value = "8080")]
        port: u16,
        #[arg(short, long)]
        username: Option<String>,
    },
}

fn main() {
    let args = Args::parse();
    
    match args.command {
        Commands::Server { host, port } => {
            println!("Starting server on {}:{}", host, port);
            println!("Use: cargo run --bin ruggine-server for the dedicated server binary");
        }
        Commands::Client { host, port, username } => {
            println!("Starting client, connecting to {}:{}", host, port);
            if let Some(username) = username {
                println!("Username: {}", username);
            }
            println!("Use: cargo run --bin ruggine-client for the dedicated client binary");
        }
    }
}
