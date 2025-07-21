use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;
use log::{info, warn};

// Definizioni locali per ora (poi useremo i modelli comuni)
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Group {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub created_by: Uuid,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub members: Vec<Uuid>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct GroupInvite {
    pub id: Uuid,
    pub group_id: Uuid,
    pub inviter_id: Uuid,
    pub invitee_id: Uuid,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub status: InviteStatus,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum InviteStatus {
    Pending,
    Accepted,
    Rejected,
    Expired,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Message {
    pub id: Uuid,
    pub sender_id: Uuid,
    pub group_id: Option<Uuid>,
    pub content: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub message_type: MessageType,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum MessageType {
    Text,
    SystemNotification,
    UserJoined,
    UserLeft,
}

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
}

impl ChatManager {
    pub fn new() -> Self {
        Self {
            users: Arc::new(RwLock::new(HashMap::new())),
            usernames: Arc::new(RwLock::new(HashMap::new())),
            groups: Arc::new(RwLock::new(HashMap::new())),
            group_names: Arc::new(RwLock::new(HashMap::new())),
            invites: Arc::new(RwLock::new(HashMap::new())),
            messages: Arc::new(RwLock::new(Vec::new())),
        }
    }
    
    pub async fn register_user(&self, username: String, addr: SocketAddr) -> Result<Uuid, String> {
        // Controlla se l'username è già in uso
        {
            let usernames = self.usernames.read().await;
            if usernames.contains_key(&username) {
                return Err("Username already taken".to_string());
            }
        }
        
        // Crea nuovo utente
        let user_id = Uuid::new_v4();
        let user = ConnectedUser {
            id: user_id,
            username: username.clone(),
            addr,
            connected_at: chrono::Utc::now(),
        };
        
        // Aggiungi ai mapping
        {
            let mut users = self.users.write().await;
            let mut usernames = self.usernames.write().await;
            
            users.insert(user_id, user);
            usernames.insert(username.clone(), user_id);
        }
        
        info!("User registered: {} ({})", username, user_id);
        Ok(user_id)
    }
    
    pub async fn user_disconnected(&self, user_id: Uuid) {
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
        // Controlla se il gruppo esiste già
        {
            let group_names = self.group_names.read().await;
            if group_names.contains_key(&group_name) {
                return Err("Group name already exists".to_string());
            }
        }

        let group_id = Uuid::new_v4();
        let group = Group {
            id: group_id,
            name: group_name.clone(),
            description: None,
            created_by: creator_id,
            created_at: chrono::Utc::now(),
            members: vec![creator_id],
        };

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
        let groups = self.groups.read().await;
        groups.values()
            .filter(|group| group.members.contains(&user_id))
            .map(|group| group.name.clone())
            .collect()
    }

    pub async fn invite_to_group(&self, inviter_id: Uuid, target_username: String, group_name: String) -> Result<Uuid, String> {
        // Trova l'utente target
        let target_id = {
            let usernames = self.usernames.read().await;
            match usernames.get(&target_username) {
                Some(&id) => id,
                None => return Err("User not found".to_string()),
            }
        };

        // Trova il gruppo
        let group_id = {
            let group_names = self.group_names.read().await;
            match group_names.get(&group_name) {
                Some(&id) => id,
                None => return Err("Group not found".to_string()),
            }
        };

        // Verifica che l'inviter sia membro del gruppo
        {
            let groups = self.groups.read().await;
            if let Some(group) = groups.get(&group_id) {
                if !group.members.contains(&inviter_id) {
                    return Err("You are not a member of this group".to_string());
                }
                
                if group.members.contains(&target_id) {
                    return Err("User is already a member of this group".to_string());
                }
            }
        }

        let invite_id = Uuid::new_v4();
        let invite = GroupInvite {
            id: invite_id,
            group_id,
            inviter_id,
            invitee_id: target_id,
            created_at: chrono::Utc::now(),
            status: InviteStatus::Pending,
        };

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

        // Verifica che l'utente sia membro del gruppo
        {
            let groups = self.groups.read().await;
            if let Some(group) = groups.get(&group_id) {
                if !group.members.contains(&sender_id) {
                    return Err("You are not a member of this group".to_string());
                }
            }
        }

        let message = Message {
            id: Uuid::new_v4(),
            sender_id,
            group_id: Some(group_id),
            content: content.clone(),
            timestamp: chrono::Utc::now(),
            message_type: MessageType::Text,
        };

        {
            let mut messages = self.messages.write().await;
            messages.push(message);
        }

        info!("Message sent to group '{}' by user {}: {}", group_name, sender_id, content);
        Ok(())
    }

    pub async fn send_private_message(&self, sender_id: Uuid, target_username: String, content: String) -> Result<(), String> {
        // Trova l'utente target
        let target_id = {
            let usernames = self.usernames.read().await;
            match usernames.get(&target_username) {
                Some(&id) => id,
                None => return Err("User not found".to_string()),
            }
        };

        let message = Message {
            id: Uuid::new_v4(),
            sender_id,
            group_id: None, // None indica messaggio privato
            content: content.clone(),
            timestamp: chrono::Utc::now(),
            message_type: MessageType::Text,
        };

        {
            let mut messages = self.messages.write().await;
            messages.push(message);
        }

        info!("Private message sent from {} to {}: {}", sender_id, target_username, content);
        Ok(())
    }

    pub async fn get_user_invites(&self, user_id: Uuid) -> Vec<String> {
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

    pub async fn accept_invite(&self, user_id: Uuid, invite_id: Uuid) -> Result<String, String> {
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

            invite.status = InviteStatus::Accepted;

            // Aggiungi l'utente al gruppo
            let mut groups = self.groups.write().await;
            let group = groups.get_mut(&invite.group_id)
                .ok_or("Group not found")?;

            if !group.members.contains(&user_id) {
                group.members.push(user_id);
            }

            group.name.clone()
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