use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;
use log::{info, warn};
use crate::database::DatabaseManager;

// Import dei modelli comuni
use crate::common::models::{Group, GroupInvite, InviteStatus, Message, MessageType, User};

// Struttura per utenti connessi (specifica del server)
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ConnectedUser {
    pub id: Uuid,
    pub username: String,
    pub addr: SocketAddr,
    pub connected_at: chrono::DateTime<chrono::Utc>,
}


pub struct ChatManager {
    users: Arc<RwLock<HashMap<Uuid, ConnectedUser>>>,
    usernames: Arc<RwLock<HashMap<String, Uuid>>>, // username -> user_id mapping
    groups: Arc<RwLock<HashMap<Uuid, Group>>>, // group_id -> group
    group_names: Arc<RwLock<HashMap<String, Uuid>>>, // group_name -> group_id
    invites: Arc<RwLock<HashMap<Uuid, GroupInvite>>>, // invite_id -> invite
    messages: Arc<RwLock<Vec<Message>>>, // All messages
    db_manager: Arc<DatabaseManager>, // Database manager
}

impl ChatManager {
    pub fn new(db_manager: Arc<DatabaseManager>) -> Self {
        Self {
            users: Arc::new(RwLock::new(HashMap::new())),
            usernames: Arc::new(RwLock::new(HashMap::new())),
            groups: Arc::new(RwLock::new(HashMap::new())),
            group_names: Arc::new(RwLock::new(HashMap::new())),
            invites: Arc::new(RwLock::new(HashMap::new())),
            messages: Arc::new(RwLock::new(Vec::new())),
            db_manager,
        }
    }
    
    pub async fn register_user(&self, username: String, addr: SocketAddr) -> Result<Uuid, String> {
        // Controlla se l'username è già in uso nel database
        match self.db_manager.get_user_by_username(&username).await {
            Ok(Some(_)) => return Err("Username already taken".to_string()),
            Ok(None) => {}, // Username disponibile
            Err(e) => return Err(format!("Database error: {}", e)),
        }
        
        // Crea nuovo utente
        let user_id = Uuid::new_v4();
        
        // Salva utente nel database
        let user = User {
            id: user_id,
            username: username.clone(),
            created_at: chrono::Utc::now(),
            is_online: true,
        };
        
        if let Err(e) = self.db_manager.create_user(&user).await {
            return Err(format!("Failed to create user in database: {}", e));
        }
        
        // Crea ConnectedUser per la gestione in memoria della connessione
        let connected_user = ConnectedUser {
            id: user_id,
            username: username.clone(),
            addr,
            connected_at: chrono::Utc::now(),
        };
        
        // Aggiungi ai mapping in memoria per gestione connessioni
        {
            let mut users = self.users.write().await;
            let mut usernames = self.usernames.write().await;
            
            users.insert(user_id, connected_user);
            usernames.insert(username.clone(), user_id);
        }
        
        info!("User registered: {} ({})", username, user_id);
        Ok(user_id)
    }
    
    pub async fn user_disconnected(&self, user_id: Uuid) {
        // Aggiorna lo stato nel database
        if let Err(e) = self.db_manager.update_user_online_status(user_id, false).await {
            warn!("Failed to update user offline status in database: {}", e);
        }
        
        // Rimuovi dalla gestione in memoria
        let mut users = self.users.write().await;
        let mut usernames = self.usernames.write().await;
        
        if let Some(user) = users.remove(&user_id) {
            usernames.remove(&user.username);
            info!("User disconnected: {} ({})", user.username, user_id);
        }
    }
    
    pub async fn list_online_users(&self) -> Vec<String> {
        let users = self.users.read().await;
        users.values()
            .map(|user| user.username.clone())
            .collect()
    }
    
    pub async fn get_user_count(&self) -> usize {
        self.users.read().await.len()
    }

    // === GROUP MANAGEMENT ===
    pub async fn create_group(&self, creator_id: Uuid, group_name: String) -> Result<Uuid, String> {
        // Crea il gruppo nel database
        let group_id = Uuid::new_v4();
        let group = Group {
            id: group_id,
            name: group_name.clone(),
            description: None,
            created_by: creator_id,
            created_at: chrono::Utc::now(),
            members: vec![creator_id],
        };
        
        // Salva nel database
        if let Err(e) = self.db_manager.create_group(&group).await {
            return Err(format!("Failed to create group in database: {}", e));
        }
        
        // Aggiorna cache in memoria
        {
            let mut groups = self.groups.write().await;
            let mut group_names = self.group_names.write().await;
            
            groups.insert(group_id, group);
            group_names.insert(group_name.clone(), group_id);
        }

        info!("Group '{}' created by user {}", group_name, creator_id);
        Ok(group_id)
    }

    pub async fn get_user_groups(&self, user_id: Uuid) -> Vec<String> {
        // Ottieni gruppi dal database
        match self.db_manager.get_user_groups(user_id).await {
            Ok(groups) => groups.into_iter().map(|group| group.name).collect(),
            Err(e) => {
                warn!("Failed to get user groups from database: {}", e);
                // Fallback alla cache in memoria
                let groups = self.groups.read().await;
                groups.values()
                    .filter(|group| group.members.contains(&user_id))
                    .map(|group| group.name.clone())
                    .collect()
            }
        }
    }

    pub async fn invite_to_group(&self, inviter_id: Uuid, target_username: String, group_name: String) -> Result<Uuid, String> {
        // Trova l'utente target nel database
        let target_user = match self.db_manager.get_user_by_username(&target_username).await {
            Ok(Some(user)) => user,
            Ok(None) => return Err("User not found".to_string()),
            Err(e) => return Err(format!("Database error: {}", e)),
        };
        
        // Trova il gruppo (prima prova in memoria, poi database)
        let group_id = {
            let group_names = self.group_names.read().await;
            if let Some(&id) = group_names.get(&group_name) {
                id
            } else {
                // Fallback: cerca nel database se non è in cache
                return Err("Group not found".to_string());
            }
        };

        // Verifica che l'inviter sia membro del gruppo (usa database)
        let group_members = match self.db_manager.get_group_members(group_id).await {
            Ok(members) => members,
            Err(e) => return Err(format!("Failed to get group members: {}", e)),
        };
        
        if !group_members.contains(&inviter_id) {
            return Err("You are not a member of this group".to_string());
        }
        
        if group_members.contains(&target_user.id) {
            return Err("User is already a member of this group".to_string());
        }

        // Crea l'invito
        let invite_id = Uuid::new_v4();
        let invite = GroupInvite {
            id: invite_id,
            group_id,
            inviter_id,
            invitee_id: target_user.id,
            created_at: chrono::Utc::now(),
            expires_at: Some(chrono::Utc::now() + chrono::Duration::days(7)),
            status: InviteStatus::Pending,
            responded_at: None,
        };

        // Salva nel database
        if let Err(e) = self.db_manager.create_group_invite(&invite).await {
            return Err(format!("Failed to create invite in database: {}", e));
        }

        // Aggiorna cache in memoria
        {
            let mut invites = self.invites.write().await;
            invites.insert(invite_id, invite);
        }

        info!("Invite sent: {} invited {} to group {}", inviter_id, target_username, group_name);
        Ok(invite_id)
    }

    pub async fn send_group_message(&self, sender_id: Uuid, group_name: String, content: String) -> Result<(), String> {
        // Trova il gruppo
        let group_id = {
            let group_names = self.group_names.read().await;
            match group_names.get(&group_name) {
                Some(&id) => id,
                None => return Err("Group not found".to_string()),
            }
        };

        // Verifica che l'utente sia membro del gruppo (usa database)
        let group_members = match self.db_manager.get_group_members(group_id).await {
            Ok(members) => members,
            Err(e) => return Err(format!("Failed to get group members: {}", e)),
        };
        
        if !group_members.contains(&sender_id) {
            return Err("You are not a member of this group".to_string());
        }

        let message = Message {
            id: Uuid::new_v4(),
            sender_id,
            group_id: Some(group_id),
            content: content.clone(),
            timestamp: chrono::Utc::now(),
            message_type: MessageType::Text,
        };

        // Salva nel database
        if let Err(e) = self.db_manager.save_message(&message).await {
            warn!("Failed to save message to database: {}", e);
        }

        // Aggiorna cache in memoria
        {
            let mut messages = self.messages.write().await;
            messages.push(message);
        }

        info!("Message sent to group '{}' by user {}: {}", group_name, sender_id, content);
        Ok(())
    }

    pub async fn send_private_message(&self, sender_id: Uuid, target_username: String, content: String) -> Result<(), String> {
        // Trova l'utente target nel database
        let target_user = match self.db_manager.get_user_by_username(&target_username).await {
            Ok(Some(user)) => user,
            Ok(None) => return Err("User not found".to_string()),
            Err(e) => return Err(format!("Database error: {}", e)),
        };

        let message = Message {
            id: Uuid::new_v4(),
            sender_id,
            group_id: None, // None indica messaggio privato
            content: content.clone(),
            timestamp: chrono::Utc::now(),
            message_type: MessageType::Text,
        };

        // Salva nel database usando la funzione specifica per messaggi diretti
        let message_id = match self.db_manager.save_direct_message(sender_id, target_user.id, &content, MessageType::Text).await {
            Ok(id) => id,
            Err(e) => {
                warn!("Failed to save private message to database: {}", e);
                message.id // Usa l'ID generato localmente come fallback
            }
        };

        // Aggiorna cache in memoria
        {
            let mut messages = self.messages.write().await;
            messages.push(message);
        }

        info!("Private message sent from {} to {}: {}", sender_id, target_username, content);
        Ok(())
    }

    pub async fn get_user_invites(&self, user_id: Uuid) -> Vec<String> {
        // Ottieni inviti dal database
        match self.db_manager.get_pending_invites(user_id).await {
            Ok(invites) => {
                let mut result = Vec::new();
                for invite in invites {
                    // Per ogni invito, ottieni le informazioni del gruppo e dell'inviter
                    if let Ok(groups) = self.db_manager.get_user_groups(invite.inviter_id).await {
                        if let Some(group) = groups.iter().find(|g| g.id == invite.group_id) {
                            if let Ok(Some(inviter)) = self.db_manager.get_user_by_username("").await {
                                // Nota: dovremmo avere una funzione get_user_by_id nel database
                                let info = format!("ID: {} | Group: '{}' | From: {} | Date: {}", 
                                    invite.id, 
                                    group.name, 
                                    "unknown", // Temporaneo finché non aggiungiamo get_user_by_id
                                    invite.created_at.format("%Y-%m-%d %H:%M")
                                );
                                result.push(info);
                            }
                        }
                    }
                }
                result
            }
            Err(e) => {
                warn!("Failed to get user invites from database: {}", e);
                // Fallback alla cache in memoria
                let invites = self.invites.read().await;
                let groups = self.groups.read().await;
                let users = self.users.read().await;

                invites.values()
                    .filter(|invite| invite.invitee_id == user_id && matches!(invite.status, InviteStatus::Pending))
                    .filter_map(|invite| {
                        let group = groups.get(&invite.group_id)?;
                        let inviter = users.get(&invite.inviter_id)?;
                        Some(format!("ID: {} | Group: '{}' | From: {} | Date: {}", 
                            invite.id, 
                            group.name, 
                            inviter.username,
                            invite.created_at.format("%Y-%m-%d %H:%M")
                        ))
                    })
                    .collect()
            }
        }
    }

    pub async fn accept_invite(&self, user_id: Uuid, invite_id: Uuid) -> Result<String, String> {
        // Accetta l'invito nel database
        if let Err(e) = self.db_manager.accept_group_invite(invite_id).await {
            return Err(format!("Failed to accept invite in database: {}", e));
        }
        
        // Ottieni le informazioni dell'invito per il nome del gruppo
        let group_name = {
            let mut invites = self.invites.write().await;
            if let Some(invite) = invites.get_mut(&invite_id) {
                if invite.invitee_id != user_id {
                    return Err("This invite is not for you".to_string());
                }

                if !matches!(invite.status, InviteStatus::Pending) {
                    return Err("Invite is no longer pending".to_string());
                }

                invite.status = InviteStatus::Accepted;
                invite.responded_at = Some(chrono::Utc::now());

                // Aggiungi l'utente al gruppo nella cache
                let mut groups = self.groups.write().await;
                if let Some(group) = groups.get_mut(&invite.group_id) {
                    if !group.members.contains(&user_id) {
                        group.members.push(user_id);
                    }
                    group.name.clone()
                } else {
                    "Unknown Group".to_string()
                }
            } else {
                return Err("Invite not found".to_string());
            }
        };

        info!("User {} accepted invite to group '{}'", user_id, group_name);
        Ok(group_name)
    }

    pub async fn reject_invite(&self, user_id: Uuid, invite_id: Uuid) -> Result<String, String> {
        let group_name = {
            let mut invites = self.invites.write().await;
            let invite = invites.get_mut(&invite_id)
                .ok_or("Invite not found")?;

            if invite.invitee_id != user_id {
                return Err("This invite is not for you".to_string());
            }

            if !matches!(invite.status, InviteStatus::Pending) {
                return Err("Invite is no longer pending".to_string());
            }

            invite.status = InviteStatus::Rejected;
            invite.responded_at = Some(chrono::Utc::now());

            let groups = self.groups.read().await;
            let group = groups.get(&invite.group_id)
                .ok_or("Group not found")?;

            group.name.clone()
        };

        info!("User {} rejected invite to group '{}'", user_id, group_name);
        Ok(group_name)
    }

    // === PERFORMANCE & PERSISTENCE ===
    pub async fn save_to_file(&self) -> Result<(), String> {
        use std::fs;
        use serde_json;

        let groups_data = {
            let groups = self.groups.read().await;
            groups.iter().map(|(k, v)| (k.to_string(), v.clone())).collect::<std::collections::HashMap<String, Group>>()
        };

        let invites_data = {
            let invites = self.invites.read().await;
            invites.iter().map(|(k, v)| (k.to_string(), v.clone())).collect::<std::collections::HashMap<String, GroupInvite>>()
        };

        let messages_data = {
            let messages = self.messages.read().await;
            messages.clone()
        };

        let data = serde_json::json!({
            "groups": groups_data,
            "invites": invites_data,
            "messages": messages_data
        });

        fs::write("ruggine_data.json", serde_json::to_string_pretty(&data).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
        info!("Server data saved to ruggine_data.json");
        Ok(())
    }

    pub async fn get_performance_metrics(&self) -> (usize, usize, usize) {
        let user_count = self.users.read().await.len();
        let group_count = self.groups.read().await.len();
        let message_count = self.messages.read().await.len();
        (user_count, group_count, message_count)
    }
}