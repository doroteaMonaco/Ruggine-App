use std::sync::{Arc, Mutex};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, BufWriter};
use tokio::net::TcpStream;
use tokio::sync::mpsc as tokio_mpsc;
use log::{info, warn, error};

#[derive(Debug, Clone)]
pub enum NetworkMessage {
    Connected,
    Disconnected,
    ServerResponse(String),
    Error(String),
}

#[derive(Debug, Clone)]
pub enum NetworkCommand {
    Connect(String, u16),
    Disconnect,
    SendMessage(String),
}

pub struct NetworkManager {
    command_sender: Sender<NetworkCommand>,
    message_receiver: Arc<Mutex<Receiver<NetworkMessage>>>,
    connected: Arc<Mutex<bool>>,
}

impl NetworkManager {
    pub fn new() -> Self {
        let (cmd_tx, cmd_rx) = mpsc::channel();
        let (msg_tx, msg_rx) = mpsc::channel();
        let connected = Arc::new(Mutex::new(false));
        let connected_clone = Arc::clone(&connected);
        
        // Spawn network handling thread
        thread::spawn(move || {
            Self::network_thread(cmd_rx, msg_tx, connected_clone);
        });
        
        Self {
            command_sender: cmd_tx,
            message_receiver: Arc::new(Mutex::new(msg_rx)),
            connected,
        }
    }
    
    pub fn send_command(&self, command: NetworkCommand) {
        if let Err(e) = self.command_sender.send(command) {
            error!("Failed to send network command: {}", e);
        }
    }
    
    pub fn get_messages(&self) -> Vec<NetworkMessage> {
        let mut messages = Vec::new();
        if let Ok(receiver) = self.message_receiver.lock() {
            while let Ok(msg) = receiver.try_recv() {
                messages.push(msg);
            }
        }
        messages
    }
    
    pub fn is_connected(&self) -> bool {
        self.connected.lock().map(|guard| *guard).unwrap_or(false)
    }
    
    fn network_thread(
        cmd_rx: Receiver<NetworkCommand>,
        msg_tx: Sender<NetworkMessage>,
        connected: Arc<Mutex<bool>>,
    ) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let mut connection_task: Option<tokio::task::JoinHandle<()>> = None;
            let (internal_cmd_tx, mut internal_cmd_rx) = tokio_mpsc::unbounded_channel();
            
            loop {
                // Check for external commands with timeout
                match cmd_rx.recv_timeout(Duration::from_millis(100)) {
                    Ok(command) => {
                        match command {
                            NetworkCommand::Connect(host, port) => {
                                info!("Attempting to connect to {}:{}", host, port);
                                
                                // Cancel existing connection if any
                                if let Some(task) = connection_task.take() {
                                    task.abort();
                                }
                                
                                let msg_tx_clone = msg_tx.clone();
                                let connected_clone = Arc::clone(&connected);
                                let mut cmd_rx_internal = internal_cmd_rx;
                                
                                // Create new command channel for this connection
                                let (new_tx, new_rx) = tokio_mpsc::unbounded_channel();
                                internal_cmd_rx = new_rx;
                                
                                // Start connection task
                                connection_task = Some(tokio::spawn(async move {
                                    Self::handle_connection(host, port, cmd_rx_internal, msg_tx_clone, connected_clone).await;
                                }));
                            }
                            NetworkCommand::Disconnect => {
                                if let Some(task) = connection_task.take() {
                                    task.abort();
                                    *connected.lock().unwrap() = false;
                                    let _ = msg_tx.send(NetworkMessage::Disconnected);
                                }
                            }
                            NetworkCommand::SendMessage(text) => {
                                let _ = internal_cmd_tx.send(text);
                            }
                        }
                    }
                    Err(mpsc::RecvTimeoutError::Timeout) => {
                        // Continue loop - this is normal
                    }
                    Err(mpsc::RecvTimeoutError::Disconnected) => {
                        error!("Command channel disconnected");
                        break;
                    }
                }
                
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
        });
    }
    
    async fn handle_connection(
        host: String,
        port: u16,
        mut cmd_rx: tokio_mpsc::UnboundedReceiver<String>,
        msg_tx: Sender<NetworkMessage>,
        connected: Arc<Mutex<bool>>,
    ) {
        match TcpStream::connect(format!("{}:{}", host, port)).await {
            Ok(stream) => {
                info!("Successfully connected to server");
                *connected.lock().unwrap() = true;
                let _ = msg_tx.send(NetworkMessage::Connected);
                
                let (reader, writer) = stream.into_split();
                let mut buf_reader = BufReader::new(reader);
                let mut buf_writer = BufWriter::new(writer);
                
                let msg_tx_read = msg_tx.clone();
                let connected_read = Arc::clone(&connected);
                
                // Spawn reading task
                let mut read_task = tokio::spawn(async move {
                    let mut line = String::new();
                    loop {
                        line.clear();
                        match buf_reader.read_line(&mut line).await {
                            Ok(0) => {
                                info!("Server closed connection");
                                break;
                            }
                            Ok(_) => {
                                let response = line.trim().to_string();
                                if !response.is_empty() {
                                    let _ = msg_tx_read.send(NetworkMessage::ServerResponse(response));
                                }
                            }
                            Err(e) => {
                                error!("Error reading from server: {}", e);
                                break;
                            }
                        }
                    }
                    *connected_read.lock().unwrap() = false;
                    let _ = msg_tx_read.send(NetworkMessage::Disconnected);
                });
                
                // Handle writing
                loop {
                    tokio::select! {
                        msg = cmd_rx.recv() => {
                            match msg {
                                Some(text) => {
                                    if let Err(e) = buf_writer.write_all(format!("{}\n", text).as_bytes()).await {
                                        error!("Failed to send message: {}", e);
                                        let _ = msg_tx.send(NetworkMessage::Error(format!("Send failed: {}", e)));
                                        break;
                                    } else {
                                        let _ = buf_writer.flush().await;
                                    }
                                }
                                None => break,
                            }
                        }
                        result = &mut read_task => {
                            // Reading task finished, connection is closed
                            match result {
                                Ok(_) => info!("Read task completed normally"),
                                Err(e) => error!("Read task error: {}", e),
                            }
                            break;
                        }
                    }
                }
                
                read_task.abort();
            }
            Err(e) => {
                error!("Failed to connect: {}", e);
                let _ = msg_tx.send(NetworkMessage::Error(format!("Connection failed: {}", e)));
            }
        }
        
        *connected.lock().unwrap() = false;
    }
}
