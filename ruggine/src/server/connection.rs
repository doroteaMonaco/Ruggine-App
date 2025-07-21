use crate::chat_manager::ChatManager;
use log::{info, warn, error, debug};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, BufWriter};
use tokio::net::TcpStream;
use uuid::Uuid;

pub struct ClientConnection {
    stream: TcpStream,
    addr: SocketAddr,
    user_id: Option<Uuid>,
    username: Option<String>,
}

async fn send_welcome(writer: &mut BufWriter<tokio::net::tcp::WriteHalf<'_>>) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let welcome_msg = r#"
=== Welcome to Ruggine Chat Server ===
Commands:
  /register <username>  - Register with a username
  /help                 - Show this help
  /quit                - Disconnect

Please register first: /register <your_username>
"#;
    writer.write_all(welcome_msg.as_bytes()).await?;
    writer.flush().await?;
    Ok(())
}

async fn send_success(writer: &mut BufWriter<tokio::net::tcp::WriteHalf<'_>>, message: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    writer.write_all(format!("OK: {}\n", message).as_bytes()).await?;
    writer.flush().await?;
    Ok(())
}

async fn send_error(writer: &mut BufWriter<tokio::net::tcp::WriteHalf<'_>>, message: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    writer.write_all(format!("ERROR: {}\n", message).as_bytes()).await?;
    writer.flush().await?;
    Ok(())
}

async fn send_help(writer: &mut BufWriter<tokio::net::tcp::WriteHalf<'_>>) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let help_msg = r#"
=== Ruggine Chat Commands ===
/register <username>     - Register with a username
/users                   - List online users
/create_group <name>     - Create a new group
/my_groups               - List your groups
/invite <username> <group_name> - Invite user to group
/accept_invite <invite_id> - Accept a group invite
/reject_invite <invite_id> - Reject a group invite
/my_invites              - List pending invites
/join_group <group_name> - Join a group chat
/leave_group <group_name> - Leave a group
/send <group_name> <message> - Send message to group
/send_private <username> <message> - Send private message
/save                    - Save server data to file (admin only)
/help                    - Show this help
/quit                    - Disconnect

Example usage:
  /create_group friends
  /invite alice friends
  /send friends Hello everyone!
"#;
    writer.write_all(help_msg.as_bytes()).await?;
    writer.flush().await?;
    Ok(())
}

async fn process_command(
    command: &str,
    chat_manager: &Arc<ChatManager>,
    writer: &mut BufWriter<tokio::net::tcp::WriteHalf<'_>>,
    addr: SocketAddr,
    user_id: &mut Option<Uuid>,
    username: &mut Option<String>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let parts: Vec<&str> = command.split_whitespace().collect();
    
    if parts.is_empty() {
        return Ok(());
    }
    
    match parts[0] {
        "/register" => {
            if parts.len() != 2 {
                send_error(writer, "Usage: /register <username>").await?;
                return Ok(());
            }
            
            let new_username = parts[1].to_string();
            match chat_manager.register_user(new_username.clone(), addr).await {
                Ok(new_user_id) => {
                    *user_id = Some(new_user_id);
                    *username = Some(new_username.clone());
                    send_success(writer, &format!("Registered as: {}", new_username)).await?;
                    info!("User {} registered from {}", new_username, addr);
                }
                Err(e) => {
                    send_error(writer, &format!("Registration failed: {}", e)).await?;
                }
            }
        }
        "/help" => {
            send_help(writer).await?;
        }
        "/save" => {
            if user_id.is_none() {
                send_error(writer, "Please register first").await?;
                return Ok(());
            }
            
            match chat_manager.save_to_file().await {
                Ok(_) => {
                    send_success(writer, "Server data saved to ruggine_data.json").await?;
                }
                Err(e) => {
                    send_error(writer, &format!("Failed to save data: {}", e)).await?;
                }
            }
        }
        "/quit" => {
            send_success(writer, "Goodbye!").await?;
            return Err("DISCONNECT_REQUESTED".into());
        }
        "/users" => {
            if user_id.is_none() {
                send_error(writer, "Please register first").await?;
                return Ok(());
            }
            
            let users = chat_manager.list_online_users().await;
            let user_list = users.join(", ");
            send_success(writer, &format!("Online users: {}", user_list)).await?;
        }
        "/create_group" => {
            if user_id.is_none() {
                send_error(writer, "Please register first").await?;
                return Ok(());
            }
            if parts.len() != 2 {
                send_error(writer, "Usage: /create_group <group_name>").await?;
                return Ok(());
            }
            
            let group_name = parts[1].to_string();
            match chat_manager.create_group(user_id.unwrap(), group_name.clone()).await {
                Ok(_) => {
                    send_success(writer, &format!("Group '{}' created successfully", group_name)).await?;
                }
                Err(e) => {
                    send_error(writer, &format!("Failed to create group: {}", e)).await?;
                }
            }
        }
        "/my_groups" => {
            if user_id.is_none() {
                send_error(writer, "Please register first").await?;
                return Ok(());
            }
            
            let groups = chat_manager.get_user_groups(user_id.unwrap()).await;
            if groups.is_empty() {
                send_success(writer, "You are not in any groups").await?;
            } else {
                let group_list = groups.join(", ");
                send_success(writer, &format!("Your groups: {}", group_list)).await?;
            }
        }
        "/invite" => {
            if user_id.is_none() {
                send_error(writer, "Please register first").await?;
                return Ok(());
            }
            if parts.len() != 3 {
                send_error(writer, "Usage: /invite <username> <group_name>").await?;
                return Ok(());
            }
            
            let target_username = parts[1].to_string();
            let group_name = parts[2].to_string();
            
            match chat_manager.invite_to_group(user_id.unwrap(), target_username.clone(), group_name.clone()).await {
                Ok(invite_id) => {
                    send_success(writer, &format!("Invitation sent to {} for group '{}' (ID: {})", target_username, group_name, invite_id)).await?;
                }
                Err(e) => {
                    send_error(writer, &format!("Failed to send invite: {}", e)).await?;
                }
            }
        }
        "/send" => {
            if user_id.is_none() {
                send_error(writer, "Please register first").await?;
                return Ok(());
            }
            if parts.len() < 3 {
                send_error(writer, "Usage: /send <group_name> <message>").await?;
                return Ok(());
            }
            
            let group_name = parts[1].to_string();
            let message = parts[2..].join(" ");
            
            match chat_manager.send_group_message(user_id.unwrap(), group_name.clone(), message).await {
                Ok(_) => {
                    send_success(writer, &format!("Message sent to group '{}'", group_name)).await?;
                }
                Err(e) => {
                    send_error(writer, &format!("Failed to send message: {}", e)).await?;
                }
            }
        }
        "/send_private" => {
            if user_id.is_none() {
                send_error(writer, "Please register first").await?;
                return Ok(());
            }
            if parts.len() < 3 {
                send_error(writer, "Usage: /send_private <username> <message>").await?;
                return Ok(());
            }
            
            let target_username = parts[1].to_string();
            let message = parts[2..].join(" ");
            
            match chat_manager.send_private_message(user_id.unwrap(), target_username.clone(), message).await {
                Ok(_) => {
                    send_success(writer, &format!("Private message sent to {}", target_username)).await?;
                }
                Err(e) => {
                    send_error(writer, &format!("Failed to send private message: {}", e)).await?;
                }
            }
        }
        "/my_invites" => {
            if user_id.is_none() {
                send_error(writer, "Please register first").await?;
                return Ok(());
            }
            
            let invites = chat_manager.get_user_invites(user_id.unwrap()).await;
            if invites.is_empty() {
                send_success(writer, "You have no pending invites").await?;
            } else {
                let invite_list = invites.join("\n  ");
                send_success(writer, &format!("Your pending invites:\n  {}", invite_list)).await?;
            }
        }
        "/accept_invite" => {
            if user_id.is_none() {
                send_error(writer, "Please register first").await?;
                return Ok(());
            }
            if parts.len() != 2 {
                send_error(writer, "Usage: /accept_invite <invite_id>").await?;
                return Ok(());
            }
            
            let invite_id_str = parts[1];
            match uuid::Uuid::parse_str(invite_id_str) {
                Ok(invite_id) => {
                    match chat_manager.accept_invite(user_id.unwrap(), invite_id).await {
                        Ok(group_name) => {
                            send_success(writer, &format!("Invite accepted! You joined group '{}'", group_name)).await?;
                        }
                        Err(e) => {
                            send_error(writer, &format!("Failed to accept invite: {}", e)).await?;
                        }
                    }
                }
                Err(_) => {
                    send_error(writer, "Invalid invite ID format").await?;
                }
            }
        }
        "/reject_invite" => {
            if user_id.is_none() {
                send_error(writer, "Please register first").await?;
                return Ok(());
            }
            if parts.len() != 2 {
                send_error(writer, "Usage: /reject_invite <invite_id>").await?;
                return Ok(());
            }
            
            let invite_id_str = parts[1];
            match uuid::Uuid::parse_str(invite_id_str) {
                Ok(invite_id) => {
                    match chat_manager.reject_invite(user_id.unwrap(), invite_id).await {
                        Ok(group_name) => {
                            send_success(writer, &format!("Invite to group '{}' rejected", group_name)).await?;
                        }
                        Err(e) => {
                            send_error(writer, &format!("Failed to reject invite: {}", e)).await?;
                        }
                    }
                }
                Err(_) => {
                    send_error(writer, "Invalid invite ID format").await?;
                }
            }
        }
        cmd if cmd.starts_with('/') => {
            send_error(writer, &format!("Unknown command: {}", cmd)).await?;
        }
        _ => {
            // Messaggio normale (non implementato ancora)
            if user_id.is_none() {
                send_error(writer, "Please register first").await?;
            } else {
                send_error(writer, "Message sending not implemented yet").await?;
            }
        }
    }
    
    Ok(())
}

impl ClientConnection {
    pub fn new(stream: TcpStream, addr: SocketAddr) -> Self {
        Self {
            stream,
            addr,
            user_id: None,
            username: None,
        }
    }
    
    pub async fn handle(mut self, chat_manager: Arc<ChatManager>) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let addr = self.addr;
        info!("Handling client connection from {}", addr);
        
        let (reader, writer) = self.stream.split();
        let mut buf_reader = BufReader::new(reader);
        let mut buf_writer = BufWriter::new(writer);
        
        // Invia messaggio di benvenuto
        send_welcome(&mut buf_writer).await?;
        
        // Variabili per tracciare lo stato del client
        let mut user_id: Option<Uuid> = None;
        let mut username: Option<String> = None;
        
        // Loop principale per leggere comandi
        let mut line = String::new();
        loop {
            line.clear();
            
            match buf_reader.read_line(&mut line).await {
                Ok(0) => {
                    // Connessione chiusa dal client
                    debug!("Client {} closed connection", addr);
                    break;
                }
                Ok(_) => {
                    let trimmed = line.trim();
                    if trimmed.is_empty() {
                        continue;
                    }
                    
                    debug!("Received from {}: {}", addr, trimmed);
                    
                    if let Err(e) = process_command(trimmed, &chat_manager, &mut buf_writer, addr, &mut user_id, &mut username).await {
                        if e.to_string() == "DISCONNECT_REQUESTED" {
                            debug!("Client {} requested disconnect", addr);
                            break;
                        } else {
                            error!("Error processing command from {}: {}", addr, e);
                            send_error(&mut buf_writer, "Internal server error").await?;
                        }
                    }
                }
                Err(e) => {
                    error!("Error reading from {}: {}", addr, e);
                    break;
                }
            }
        }
        
        // Cleanup quando la connessione termina
        if let Some(uid) = user_id {
            chat_manager.user_disconnected(uid).await;
        }
        
        Ok(())
    }
}