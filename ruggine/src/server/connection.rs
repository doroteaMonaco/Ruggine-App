#![allow(dead_code)]

use crate::server::chat_manager::{ChatManager, ClientNotification};
use log::{info, error, debug};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, BufWriter};
use tokio::net::TcpStream;
use tokio::sync::mpsc;
use uuid::Uuid;

#[allow(dead_code)]
pub struct ClientConnection {
    stream: TcpStream,
    addr: SocketAddr,
}

#[allow(dead_code)]
async fn send_welcome(writer: &mut BufWriter<tokio::net::tcp::OwnedWriteHalf>) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
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

#[allow(dead_code)]
async fn send_success(writer: &mut BufWriter<tokio::net::tcp::OwnedWriteHalf>, message: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    writer.write_all(format!("OK: {}\n", message).as_bytes()).await?;
    writer.flush().await?;
    Ok(())
}

#[allow(dead_code)]
async fn send_error(writer: &mut BufWriter<tokio::net::tcp::OwnedWriteHalf>, message: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    writer.write_all(format!("ERROR: {}\n", message).as_bytes()).await?;
    writer.flush().await?;
    Ok(())
}

#[allow(dead_code)]
async fn send_help(writer: &mut BufWriter<tokio::net::tcp::OwnedWriteHalf>) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let help_msg = r#"
=== Ruggine Chat Commands ===
/register <username>     - Register with a username
/users                   - List online users
/all_users               - List all registered users (excluding yourself)
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
/private <username> <message> - Send private message (alternative)
/get_group_messages <group_name> - Get messages from a group chat
/get_private_messages <username> - Get private messages with a user
/delete_group_messages <group_id> - Delete all messages from a group chat
/delete_private_messages <username> - Delete all messages from a private chat
/save                    - Save server data to file (admin only)
/help                    - Show this help
/quit                    - Disconnect

Example usage:
  /create_group friends
  /invite alice friends
  /join_group friends
  /send friends Hello everyone!
  /get_group_messages friends
  /private alice Hi there!
  /get_private_messages alice
  /delete_group_messages <group_id>
  /delete_private_messages alice
"#;
    writer.write_all(help_msg.as_bytes()).await?;
    writer.flush().await?;
    Ok(())
}

#[allow(dead_code)]
async fn process_command(
    command: &str,
    chat_manager: &Arc<ChatManager>,
    writer: &mut BufWriter<tokio::net::tcp::OwnedWriteHalf>,
    addr: SocketAddr,
    user_id: &mut Option<Uuid>,
    username: &mut Option<String>,
    notification_tx: &mpsc::UnboundedSender<ClientNotification>,
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
                    
                    // Registra il canale di notifica per questo utente
                    chat_manager.register_notification_channel(new_user_id, notification_tx.clone()).await;
                    
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
        "/all_users" => {
            if user_id.is_none() {
                send_error(writer, "Please register first").await?;
                return Ok(());
            }
            
            // Ottieni il nome utente corrente per escluderlo
            let current_username = if let Some(uid) = user_id {
                chat_manager.get_username_by_id(&uid).await
            } else {
                None
            };
            
            let users = chat_manager.list_all_users(current_username.as_deref()).await;
            let user_list = users.join(", ");
            send_success(writer, &format!("All users: {}", user_list)).await?;
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
            match chat_manager.create_group(group_name.clone(), user_id.unwrap()).await {
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
            
            let groups_with_id = chat_manager.get_user_groups_with_id(user_id.unwrap()).await;
            if groups_with_id.is_empty() {
                send_success(writer, "You are not in any groups").await?;
            } else {
                let mut response = String::from("Your groups:\n");
                for (group_id, group_name) in groups_with_id {
                    response.push_str(&format!("  ID: {} | Name: '{}'\n", group_id, group_name));
                }
                send_success(writer, &response.trim()).await?;
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
        "/invite_by_id" => {
            if user_id.is_none() {
                send_error(writer, "Please register first").await?;
                return Ok(());
            }
            if parts.len() != 3 {
                send_error(writer, "Usage: /invite_by_id <username> <group_id>").await?;
                return Ok(());
            }
            
            let target_username = parts[1].to_string();
            let group_id_str = parts[2];
            
            let group_id = match uuid::Uuid::parse_str(group_id_str) {
                Ok(id) => id,
                Err(_) => {
                    send_error(writer, "Invalid group ID format").await?;
                    return Ok(());
                }
            };
            
            match chat_manager.invite_to_group_by_id(user_id.unwrap(), target_username.clone(), group_id).await {
                Ok(invite_id) => {
                    send_success(writer, &format!("Invitation sent to {} for group ID {} (Invite ID: {})", target_username, group_id, invite_id)).await?;
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
        "/leave_group" => {
            if user_id.is_none() {
                send_error(writer, "Please register first").await?;
                return Ok(());
            }
            if parts.len() != 2 {
                send_error(writer, "Usage: /leave_group <group_name>").await?;
                return Ok(());
            }
            
            let group_name = parts[1];
            match chat_manager.leave_group(user_id.unwrap(), group_name.to_string()).await {
                Ok(message) => {
                    send_success(writer, &message).await?;
                }
                Err(e) => {
                    send_error(writer, &format!("Failed to leave group: {}", e)).await?;
                }
            }
        }
        "/leave_group_by_id" => {
            if user_id.is_none() {
                send_error(writer, "Please register first").await?;
                return Ok(());
            }
            if parts.len() != 2 {
                send_error(writer, "Usage: /leave_group_by_id <group_id>").await?;
                return Ok(());
            }
            
            let group_id_str = parts[1];
            let group_id = match uuid::Uuid::parse_str(group_id_str) {
                Ok(id) => id,
                Err(_) => {
                    send_error(writer, "Invalid group ID format").await?;
                    return Ok(());
                }
            };
            
            match chat_manager.leave_group_by_id(user_id.unwrap(), group_id).await {
                Ok(message) => {
                    send_success(writer, &message).await?;
                }
                Err(e) => {
                    send_error(writer, &format!("Failed to leave group: {}", e)).await?;
                }
            }
        }
        "/private" => {
            if user_id.is_none() {
                send_error(writer, "Please register first").await?;
                return Ok(());
            }
            if parts.len() < 3 {
                send_error(writer, "Usage: /private <username> <message>").await?;
                return Ok(());
            }
            
            let target_username = parts[1].to_string();
            let message = parts[2..].join(" ");
            
            match chat_manager.send_private_message(user_id.unwrap(), target_username, message).await {
                Ok(_) => {
                    send_success(writer, "Private message sent").await?;
                }
                Err(e) => {
                    send_error(writer, &format!("Failed to send private message: {}", e)).await?;
                }
            }
        }
        "/delete_group_messages" => {
            if user_id.is_none() {
                send_error(writer, "Please register first").await?;
                return Ok(());
            }
            if parts.len() != 2 {
                send_error(writer, "Usage: /delete_group_messages <group_id>").await?;
                return Ok(());
            }

            let group_id_str = parts[1];
            let group_id = match uuid::Uuid::parse_str(group_id_str) {
                Ok(id) => id,
                Err(_) => {
                    send_error(writer, "Invalid group ID format").await?;
                    return Ok(());
                }
            };

            match chat_manager.delete_group_messages(user_id.unwrap(), group_id).await {
                Ok(deleted_count) => {
                    send_success(writer, &format!("Deleted {} messages from group", deleted_count)).await?;
                }
                Err(e) => {
                    send_error(writer, &format!("Failed to delete group messages: {}", e)).await?;
                }
            }
        }
        "/delete_private_messages" => {
            if user_id.is_none() {
                send_error(writer, "Please register first").await?;
                return Ok(());
            }
            if parts.len() != 2 {
                send_error(writer, "Usage: /delete_private_messages <username>").await?;
                return Ok(());
            }

            let target_username = parts[1].to_string();
            
            // Trova l'utente target
            let target_user_id = match chat_manager.get_user_id_by_username(&target_username).await {
                Some(id) => id,
                None => {
                    send_error(writer, &format!("User '{}' not found", target_username)).await?;
                    return Ok(());
                }
            };

            match chat_manager.delete_private_messages(user_id.unwrap(), target_user_id).await {
                Ok(deleted_count) => {
                    send_success(writer, &format!("Deleted {} private messages with {}", deleted_count, target_username)).await?;
                }
                Err(e) => {
                    send_error(writer, &format!("Failed to delete private messages: {}", e)).await?;
                }
            }
        }
        "/get_private_messages" => {
            if user_id.is_none() {
                send_error(writer, "Please register first").await?;
                return Ok(());
            }
            if parts.len() != 2 {
                send_error(writer, "Usage: /get_private_messages <username>").await?;
                return Ok(());
            }

            let target_username = parts[1].to_string();
            
            // Trova l'utente target
            let target_user_id = match chat_manager.get_user_id_by_username(&target_username).await {
                Some(id) => id,
                None => {
                    send_error(writer, &format!("User '{}' not found", target_username)).await?;
                    return Ok(());
                }
            };

            // Ottieni messaggi dal database
            match chat_manager.get_decrypted_direct_messages(user_id.unwrap(), target_user_id, 50).await {
                Ok(messages) => {
                    if messages.is_empty() {
                        send_success(writer, "No messages found").await?;
                    } else {
                        let mut response = String::from("Private messages:\n");
                        for msg in messages {
                            response.push_str(&format!("{}\n", msg));
                        }
                        send_success(writer, &response.trim()).await?;
                    }
                }
                Err(e) => {
                    send_error(writer, &format!("Failed to get private messages: {}", e)).await?;
                }
            }
        }
        "/get_group_messages" => {
            if user_id.is_none() {
                send_error(writer, "Please register first").await?;
                return Ok(());
            }
            if parts.len() != 2 {
                send_error(writer, "Usage: /get_group_messages <group_name>").await?;
                return Ok(());
            }

            let group_name = parts[1].to_string();
            
            // Trova il gruppo
            let group_id = match chat_manager.get_group_id_by_name(&group_name).await {
                Some(id) => id,
                None => {
                    send_error(writer, &format!("Group '{}' not found", group_name)).await?;
                    return Ok(());
                }
            };

            // Verifica che l'utente sia membro del gruppo
            match chat_manager.is_user_in_group(user_id.unwrap(), group_id).await {
                Ok(false) => {
                    send_error(writer, &format!("You are not a member of group '{}'", group_name)).await?;
                    return Ok(());
                }
                Err(e) => {
                    send_error(writer, &format!("Failed to check group membership: {}", e)).await?;
                    return Ok(());
                }
                Ok(true) => {} // Continue
            }

            // Ottieni messaggi dal database
            match chat_manager.get_decrypted_group_messages(group_id, 50).await {
                Ok(messages) => {
                    if messages.is_empty() {
                        send_success(writer, "No messages found in this group").await?;
                    } else {
                        let mut response = String::from(&format!("Messages from group '{}':\n", group_name));
                        for msg in messages {
                            response.push_str(&format!("{}\n", msg));
                        }
                        send_success(writer, &response.trim()).await?;
                    }
                }
                Err(e) => {
                    send_error(writer, &format!("Failed to get group messages: {}", e)).await?;
                }
            }
        }
        "/join_group" => {
            if user_id.is_none() {
                send_error(writer, "Please register first").await?;
                return Ok(());
            }
            if parts.len() != 2 {
                send_error(writer, "Usage: /join_group <group_name>").await?;
                return Ok(());
            }

            let group_name = parts[1].to_string();
            
            // Trova il gruppo
            let group_id = match chat_manager.get_group_id_by_name(&group_name).await {
                Some(id) => id,
                None => {
                    send_error(writer, &format!("Group '{}' not found", group_name)).await?;
                    return Ok(());
                }
            };

            // Verifica che l'utente sia membro del gruppo
            match chat_manager.is_user_in_group(user_id.unwrap(), group_id).await {
                Ok(false) => {
                    send_error(writer, &format!("You are not a member of group '{}'. You need an invitation to join.", group_name)).await?;
                    return Ok(());
                }
                Err(e) => {
                    send_error(writer, &format!("Failed to check group membership: {}", e)).await?;
                    return Ok(());
                }
                Ok(true) => {
                    send_success(writer, &format!("You have joined the group chat '{}'. Use /get_group_messages {} to see messages and /send {} <message> to send messages.", group_name, group_name, group_name)).await?;
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

#[allow(dead_code)]
impl ClientConnection {
    pub fn new(stream: TcpStream, addr: SocketAddr) -> Self {
        Self {
            stream,
            addr,
        }
    }
    
    pub async fn handle(self, chat_manager: Arc<ChatManager>) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let addr = self.addr;
        info!("Handling client connection from {}", addr);
        
        // Dividiamo lo stream in reader e writer
        let (reader, writer) = self.stream.into_split();
        let mut buf_reader = BufReader::new(reader);
        let mut buf_writer = BufWriter::new(writer);
        
        // Crea un canale per le notifiche
        let (notification_tx, mut notification_rx) = mpsc::unbounded_channel::<ClientNotification>();
        
        // Invia messaggio di benvenuto
        send_welcome(&mut buf_writer).await?;
        
        // Variabili per tracciare lo stato del client
        let mut user_id: Option<Uuid> = None;
        let mut username: Option<String> = None;
        
        // Task per gestire le notifiche in entrata
        let buf_writer = Arc::new(tokio::sync::Mutex::new(buf_writer));
        let buf_writer_for_notifications = Arc::clone(&buf_writer);
        let notification_task = tokio::spawn(async move {
            while let Some(notification) = notification_rx.recv().await {
                let mut writer = buf_writer_for_notifications.lock().await;
                match notification {
                    ClientNotification::NewPrivateMessage { from_username, .. } => {
                        let notification_msg = format!("NOTIFICATION:PRIVATE_MESSAGE:{}\n", from_username);
                        if let Err(e) = writer.write_all(notification_msg.as_bytes()).await {
                            error!("Failed to send private message notification: {}", e);
                            break;
                        }
                        if let Err(e) = writer.flush().await {
                            error!("Failed to flush notification: {}", e);
                            break;
                        }
                    }
                    ClientNotification::NewGroupMessage { group_name, from_username, .. } => {
                        let notification_msg = format!("NOTIFICATION:GROUP_MESSAGE:{}:{}\n", group_name, from_username);
                        if let Err(e) = writer.write_all(notification_msg.as_bytes()).await {
                            error!("Failed to send group message notification: {}", e);
                            break;
                        }
                        if let Err(e) = writer.flush().await {
                            error!("Failed to flush notification: {}", e);
                            break;
                        }
                    }
                }
            }
        });
        
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
                    
                    {
                        let mut writer = buf_writer.lock().await;
                        if let Err(e) = process_command(trimmed, &chat_manager, &mut *writer, addr, &mut user_id, &mut username, &notification_tx).await {
                            if e.to_string() == "DISCONNECT_REQUESTED" {
                                debug!("Client {} requested disconnect", addr);
                                break;
                            } else {
                                error!("Error processing command from {}: {}", addr, e);
                                send_error(&mut *writer, "Internal server error").await?;
                            }
                        }
                    }
                }
                Err(e) => {
                    // Distingui tra disconnessioni normali e errori veri
                    let is_normal_disconnect = match e.kind() {
                        std::io::ErrorKind::ConnectionReset | 
                        std::io::ErrorKind::ConnectionAborted |
                        std::io::ErrorKind::BrokenPipe |
                        std::io::ErrorKind::UnexpectedEof => true,
                        _ => {
                            // Controlla se è un errore Windows specifico per disconnessione
                            #[cfg(windows)]
                            {
                                if let Some(os_error) = e.raw_os_error() {
                                    // 10054 = WSAECONNRESET (connection reset by peer)
                                    // 10053 = WSAECONNABORTED (connection aborted)
                                    matches!(os_error, 10054 | 10053)
                                } else {
                                    false
                                }
                            }
                            #[cfg(not(windows))]
                            false
                        }
                    };

                    if is_normal_disconnect {
                        // Client disconnesso normalmente (anche se forzatamente)
                        debug!("Client {} disconnected: {}", addr, e);
                    } else {
                        // Errore vero del server
                        error!("Error reading from {}: {}", addr, e);
                    }
                    break;
                }
            }
        }
        
        // Cleanup quando la connessione termina
        if let Some(uid) = user_id {
            chat_manager.user_disconnected(uid).await;
        }
        
        // Termina il task di notifica
        notification_task.abort();
        
        Ok(())
    }
}