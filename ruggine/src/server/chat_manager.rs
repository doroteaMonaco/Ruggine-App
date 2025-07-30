use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;
use log::{info, warn, error};
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use chrono::{Utc, Duration};
use crate::server::database::DatabaseManager;

// Import dei modelli comuni
use crate::common::models::{Group, GroupInvite, InviteStatus, MessageType, User};
use crate::common::crypto::{EncryptedMessage, CryptoManager};

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
    encrypted_messages: Arc<RwLock<Vec<EncryptedMessage>>>, // All encrypted messages
    crypto_manager: Arc<RwLock<CryptoManager>>, // Crypto manager for encryption
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
            encrypted_messages: Arc::new(RwLock::new(Vec::new())),
            crypto_manager: Arc::new(RwLock::new(CryptoManager::new())),
            db_manager,
        }
    }
    
    pub async fn register_user(&self, username: String, addr: SocketAddr) -> Result<Uuid, String> {
        // Controlla se l'username è già in uso nel database
        match self.db_manager.get_user_by_username(&username).await {
            Ok(Some(existing_user)) => {
                // Se l'utente esiste ma è offline, può riconnettersi
                if !existing_user.is_online {
                    // Aggiorna l'utente come online
                    if let Err(e) = self.db_manager.update_user_online_status(existing_user.id, true).await {
                        return Err(format!("Failed to update user online status: {}", e));
                    }
                    
                    // Crea ConnectedUser per la gestione in memoria della connessione
                    let connected_user = ConnectedUser {
                        id: existing_user.id,
                        username: username.clone(),
                        addr,
                        connected_at: chrono::Utc::now(),
                    };
                    
                    // Aggiungi ai mapping in memoria per gestione connessioni
                    {
                        let mut users = self.users.write().await;
                        let mut usernames = self.usernames.write().await;
                        
                        users.insert(existing_user.id, connected_user);
                        usernames.insert(username.clone(), existing_user.id);
                    }
                    
                    info!("User reconnected: {} ({})", username, existing_user.id);
                    return Ok(existing_user.id);
                } else {
                    return Err("Username already taken (user is online)".to_string());
                }
            },
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
    
    pub async fn list_all_users(&self, exclude_username: Option<&str>) -> Vec<String> {
        match self.db_manager.get_all_users().await {
            Ok(users) => {
                users.into_iter()
                    .filter(|user| {
                        if let Some(exclude) = exclude_username {
                            user.username != exclude
                        } else {
                            true
                        }
                    })
                    .map(|user| user.username)
                    .collect()
            }
            Err(e) => {
                error!("Failed to get all users from database: {}", e);
                Vec::new()
            }
        }
    }
    
    pub async fn get_user_count(&self) -> usize {
        self.users.read().await.len()
    }
    
    pub async fn get_username_by_id(&self, user_id: &Uuid) -> Option<String> {
        let users = self.users.read().await;
        users.get(user_id).map(|user| user.username.clone())
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

        // Genera una chiave di crittografia per il gruppo
        {
            let mut crypto = self.crypto_manager.write().await;
            match crypto.generate_key() {
                Ok(group_key) => {
                    // Salva la chiave nel crypto manager
                    crypto.set_group_key(group_id, group_key.clone());
                    
                    // Esporta e salva la chiave nel database (crittografata)
                    match crypto.export_group_key(group_id) {
                        Ok(encoded_key) => {
                            if let Err(e) = self.db_manager.save_group_encryption_key(group_id, &encoded_key, creator_id).await {
                                warn!("Failed to save group encryption key to database: {}", e);
                            }
                        }
                        Err(e) => {
                            warn!("Failed to export group key: {}", e);
                        }
                    }
                }
                Err(e) => {
                    warn!("Failed to generate group encryption key: {}", e);
                }
            }
        }
        
        // Aggiorna cache in memoria
        {
            let mut groups = self.groups.write().await;
            let mut group_names = self.group_names.write().await;
            
            groups.insert(group_id, group);
            group_names.insert(group_name.clone(), group_id);
        }

        info!("Group '{}' created by user {} with encryption", group_name, creator_id);
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

    /// Ottieni gruppi dell'utente con ID e nome (per il nuovo sistema)
    pub async fn get_user_groups_with_id(&self, user_id: Uuid) -> Vec<(Uuid, String)> {
        match self.db_manager.get_user_groups(user_id).await {
            Ok(groups) => groups.into_iter().map(|group| (group.id, group.name)).collect(),
            Err(e) => {
                warn!("Failed to get user groups from database: {}", e);
                // Fallback alla cache in memoria
                let groups = self.groups.read().await;
                groups.values()
                    .filter(|group| group.members.contains(&user_id))
                    .map(|group| (group.id, group.name.clone()))
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

        // Verifica che l'inviter sia admin del gruppo
        let is_admin = match self.db_manager.is_user_group_admin(inviter_id, group_id).await {
            Ok(admin_status) => admin_status,
            Err(e) => return Err(format!("Failed to check admin status: {}", e)),
        };

        if !is_admin {
            return Err("Only group administrators can send invites".to_string());
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

    pub async fn send_encrypted_group_message(&self, encrypted_msg: EncryptedMessage) -> Result<(), String> {
        // Verifica che sia un messaggio di gruppo
        let group_id = encrypted_msg.group_id.ok_or("Not a group message")?;

        // Verifica che l'utente sia membro del gruppo (usa database)
        let group_members = match self.db_manager.get_group_members(group_id).await {
            Ok(members) => members,
            Err(e) => return Err(format!("Failed to get group members: {}", e)),
        };
        
        if !group_members.contains(&encrypted_msg.sender_id) {
            return Err("You are not a member of this group".to_string());
        }

        // Salva nel database
        if let Err(e) = self.db_manager.save_encrypted_message(&encrypted_msg).await {
            warn!("Failed to save encrypted message to database: {}", e);
            return Err(format!("Failed to save message: {}", e));
        }

        // Aggiorna cache in memoria
        {
            let mut messages = self.encrypted_messages.write().await;
            messages.push(encrypted_msg.clone());
        }

        info!("Encrypted group message sent by user {}", encrypted_msg.sender_id);
        Ok(())
    }

    pub async fn send_encrypted_private_message(&self, encrypted_msg: EncryptedMessage) -> Result<(), String> {
        // Verifica che sia un messaggio privato
        if encrypted_msg.group_id.is_some() {
            return Err("Not a private message".to_string());
        }

        let receiver_id = encrypted_msg.receiver_id.ok_or("No receiver specified")?;

        // Verifica che l'utente destinatario esista
        match self.db_manager.get_user_by_username("").await {
            Ok(_) => {}, // Potremmo verificare l'esistenza dell'utente, ma per ora assumiamo che il receiver_id sia valido
            Err(e) => return Err(format!("Database error: {}", e)),
        }

        // Salva nel database
        if let Err(e) = self.db_manager.save_encrypted_message(&encrypted_msg).await {
            warn!("Failed to save encrypted private message to database: {}", e);
            return Err(format!("Failed to save message: {}", e));
        }

        // Aggiorna cache in memoria
        {
            let mut messages = self.encrypted_messages.write().await;
            messages.push(encrypted_msg.clone());
        }

        info!("Encrypted private message sent from {} to {}", encrypted_msg.sender_id, receiver_id);
        Ok(())
    }

    pub async fn get_user_invites(&self, user_id: Uuid) -> Vec<String> {
        // Ottieni inviti dal database
        match self.db_manager.get_pending_invites(user_id).await {
            Ok(invites) => {
                let mut result = Vec::new();
                
                for invite in invites {
                    // Ottieni informazioni del gruppo
                    let group_name = match self.db_manager.get_group_by_id(invite.group_id).await {
                        Ok(Some(group)) => group.name,
                        Ok(None) => "Unknown Group".to_string(),
                        Err(_) => "Error loading group".to_string(),
                    };
                    
                    // Ottieni informazioni dell'inviter
                    let inviter_username = match self.db_manager.get_user_by_id(invite.inviter_id).await {
                        Ok(Some(user)) => user.username,
                        Ok(None) => "Unknown User".to_string(),
                        Err(_) => "Error loading user".to_string(),
                    };
                    
                    let info = format!("ID: {} | Group: '{}' | From: {} | Date: {}", 
                        invite.id, 
                        group_name, 
                        inviter_username,
                        invite.created_at.format("%Y-%m-%d %H:%M")
                    );
                    result.push(info);
                }
                
                info!("Retrieved {} pending invites for user {}", result.len(), user_id);
                result
            }
            Err(e) => {
                warn!("Failed to get user invites from database: {}", e);
                // Fallback alla cache in memoria solo se il database fallisce
                let invites = self.invites.read().await;
                let groups = self.groups.read().await;
                let users = self.users.read().await;

                let result: Vec<String> = invites.values()
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
                    .collect();
                    
                info!("Retrieved {} pending invites for user {} (from cache)", result.len(), user_id);
                result
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
        // Aggiorna l'invito nel database
        if let Err(e) = self.db_manager.reject_group_invite(invite_id).await {
            return Err(format!("Failed to reject invite: {}", e));
        }

        // Aggiorna la memoria locale
        let group_name = {
            let mut invites = self.invites.write().await;
            if let Some(invite) = invites.get_mut(&invite_id) {
                if invite.invitee_id != user_id {
                    return Err("This invite is not for you".to_string());
                }

                if !matches!(invite.status, InviteStatus::Pending) {
                    return Err("Invite is no longer pending".to_string());
                }

                invite.status = InviteStatus::Rejected;

                let groups = self.groups.read().await;
                if let Some(group) = groups.get(&invite.group_id) {
                    group.name.clone()
                } else {
                    "Unknown Group".to_string()
                }
            } else {
                return Err("Invite not found".to_string());
            }
        };

        info!("User {} rejected invite to group '{}'", user_id, group_name);
        Ok(group_name)
    }

    /// Ottieni gli inviti pendenti per un utente
    pub async fn get_user_pending_invites(&self, user_id: Uuid) -> Result<Vec<GroupInvite>, String> {
        match self.db_manager.get_user_pending_invites(user_id).await {
            Ok(invites) => Ok(invites),
            Err(e) => Err(format!("Database error: {}", e)),
        }
    }

    /// Lascia un gruppo (versione originale che usa nome - deprecata)
    pub async fn leave_group(&self, user_id: Uuid, group_name: String) -> Result<String, String> {
        // Trova il gruppo per nome
        let group_id = {
            let group_names = self.group_names.read().await;
            group_names.get(&group_name).copied()
                .ok_or_else(|| "Group not found".to_string())?
        };

        self.leave_group_by_id(user_id, group_id).await
    }

    /// Lascia un gruppo usando l'ID (nuovo metodo sicuro)
    pub async fn leave_group_by_id(&self, user_id: Uuid, group_id: Uuid) -> Result<String, String> {
        // Ottieni il nome del gruppo per la risposta
        let group_name = match self.db_manager.get_group_by_id(group_id).await {
            Ok(Some(group)) => group.name,
            Ok(None) => return Err("Group not found".to_string()),
            Err(e) => return Err(format!("Database error: {}", e)),
        };

        // Rimuovi l'utente dal gruppo nel database
        if let Err(e) = self.db_manager.remove_user_from_group(group_id, user_id).await {
            return Err(format!("Failed to leave group: {}", e));
        }

        info!("User {} left group '{}' (ID: {})", user_id, group_name, group_id);
        Ok(format!("Left group '{}'", group_name))
    }

    /// Invita un utente a un gruppo usando l'ID (nuovo metodo sicuro)
    pub async fn invite_to_group_by_id(&self, inviter_id: Uuid, target_username: String, group_id: Uuid) -> Result<Uuid, String> {
        // Trova l'utente target nel database
        let target_user = match self.db_manager.get_user_by_username(&target_username).await {
            Ok(Some(user)) => user,
            Ok(None) => return Err("User not found".to_string()),
            Err(e) => return Err(format!("Database error: {}", e)),
        };

        // Ottieni informazioni sul gruppo
        let group = match self.db_manager.get_group_by_id(group_id).await {
            Ok(Some(group)) => group,
            Ok(None) => return Err("Group not found".to_string()),
            Err(e) => return Err(format!("Database error: {}", e)),
        };

        // Verifica che l'inviter sia membro del gruppo
        let group_members = match self.db_manager.get_group_members(group_id).await {
            Ok(members) => members,
            Err(e) => return Err(format!("Failed to get group members: {}", e)),
        };

        if !group_members.contains(&inviter_id) {
            return Err("You are not a member of this group".to_string());
        }

        // Verifica permessi admin se necessario
        let is_admin = match self.db_manager.is_user_group_admin(inviter_id, group_id).await {
            Ok(is_admin) => is_admin,
            Err(e) => return Err(format!("Failed to check admin status: {}", e)),
        };

        if !is_admin {
            return Err("Only group admins can send invites".to_string());
        }

        // Verifica che l'utente target non sia già membro
        if group_members.contains(&target_user.id) {
            return Err("User is already a member of this group".to_string());
        }

        let invite = GroupInvite {
            id: Uuid::new_v4(),
            group_id,
            inviter_id,
            invitee_id: target_user.id,
            created_at: Utc::now(),
            expires_at: Some(Utc::now() + Duration::hours(24)), // Invito valido per 24 ore
            status: InviteStatus::Pending,
            responded_at: None,
        };

        // Salva l'invito nel database
        if let Err(e) = self.db_manager.create_group_invite(&invite).await {
            return Err(format!("Failed to create invite: {}", e));
        }

        // Aggiungi l'invito alla cache in memoria
        {
            let mut invites = self.invites.write().await;
            invites.insert(invite.id, invite.clone());
        }

        info!("Invite sent: {} invited {} to group {} (ID: {})", inviter_id, target_username, group.name, group_id);
        Ok(invite.id)
    }

    /// Funzione di compatibilità temporanea per inviare messaggi di gruppo (da rimuovere)
    pub async fn send_group_message(&self, sender_id: Uuid, group_name: String, content: String) -> Result<(), String> {
        // Trova il gruppo
        let group_id = {
            let group_names = self.group_names.read().await;
            match group_names.get(&group_name) {
                Some(&id) => id,
                None => return Err("Group not found".to_string()),
            }
        };

        // Carica la chiave del gruppo se non è già in memoria
        {
            let crypto = self.crypto_manager.read().await;
            if !crypto.has_group_key(group_id) {
                drop(crypto);
                // Prova a caricare la chiave dal database
                if let Ok(Some(encoded_key)) = self.db_manager.get_group_encryption_key(group_id).await {
                    let mut crypto_mut = self.crypto_manager.write().await;
                    if let Err(e) = crypto_mut.import_group_key(group_id, &encoded_key) {
                        warn!("Failed to import group key: {}", e);
                        return Err("Failed to load group encryption key".to_string());
                    }
                } else {
                    return Err("Group encryption key not found".to_string());
                }
            }
        }

        // Critta il messaggio usando il CryptoManager
        let encrypted_msg = {
            let crypto = self.crypto_manager.read().await;
            match crypto.encrypt_group_message(group_id, sender_id, &content, MessageType::Text) {
                Ok(msg) => msg,
                Err(e) => {
                    warn!("Failed to encrypt group message: {}", e);
                    return Err(format!("Failed to encrypt message: {}", e));
                }
            }
        };

        self.send_encrypted_group_message(encrypted_msg).await
    }

    /// Funzione di compatibilità temporanea per inviare messaggi privati (da rimuovere)
    pub async fn send_private_message(&self, sender_id: Uuid, target_username: String, content: String) -> Result<(), String> {
        // Trova l'utente target nel database
        let target_user = match self.db_manager.get_user_by_username(&target_username).await {
            Ok(Some(user)) => user,
            Ok(None) => return Err("User not found".to_string()),
            Err(e) => return Err(format!("Database error: {}", e)),
        };

        let receiver_id = target_user.id;

        // Carica o genera la chiave per la chat diretta
        {
            let crypto = self.crypto_manager.read().await;
            if !crypto.has_direct_key(sender_id, receiver_id) {
                drop(crypto);
                // Prova a caricare la chiave dal database
                match self.db_manager.get_user_encryption_key(sender_id, receiver_id).await {
                    Ok(Some(encoded_key)) => {
                        // Decodifica e importa la chiave esistente
                        let mut crypto_mut = self.crypto_manager.write().await;
                        crypto_mut.set_direct_key(sender_id, receiver_id, BASE64.decode(&encoded_key).map_err(|e| format!("Failed to decode key: {}", e))?);
                    }
                    Ok(None) => {
                        // Genera una nuova chiave per questa coppia di utenti
                        let mut crypto_mut = self.crypto_manager.write().await;
                        match crypto_mut.generate_key() {
                            Ok(new_key) => {
                                crypto_mut.set_direct_key(sender_id, receiver_id, new_key.clone());
                                // Salva la chiave nel database
                                let encoded_key = BASE64.encode(&new_key);
                                if let Err(e) = self.db_manager.save_user_encryption_key(sender_id, receiver_id, &encoded_key).await {
                                    warn!("Failed to save user encryption key: {}", e);
                                }
                            }
                            Err(e) => {
                                return Err(format!("Failed to generate encryption key: {}", e));
                            }
                        }
                    }
                    Err(e) => {
                        return Err(format!("Database error: {}", e));
                    }
                }
            }
        }

        // Critta il messaggio usando il CryptoManager
        let encrypted_msg = {
            let crypto = self.crypto_manager.read().await;
            match crypto.encrypt_direct_message(sender_id, receiver_id, &content, MessageType::Text) {
                Ok(msg) => msg,
                Err(e) => {
                    warn!("Failed to encrypt direct message: {}", e);
                    return Err(format!("Failed to encrypt message: {}", e));
                }
            }
        };

        self.send_encrypted_private_message(encrypted_msg).await
    }

    /// Ottieni i messaggi crittografati di un gruppo
    pub async fn get_encrypted_group_messages(&self, group_id: Uuid, limit: i64) -> Result<Vec<EncryptedMessage>, String> {
        match self.db_manager.get_encrypted_group_messages(group_id, limit).await {
            Ok(messages) => Ok(messages),
            Err(e) => Err(format!("Failed to get group messages: {}", e)),
        }
    }

    /// Ottieni e decritta i messaggi di un gruppo (per compatibilità con il client)
    pub async fn get_decrypted_group_messages(&self, group_id: Uuid, limit: i64) -> Result<Vec<String>, String> {
        // Carica la chiave del gruppo se necessario
        {
            let crypto = self.crypto_manager.read().await;
            if !crypto.has_group_key(group_id) {
                drop(crypto);
                if let Ok(Some(encoded_key)) = self.db_manager.get_group_encryption_key(group_id).await {
                    let mut crypto_mut = self.crypto_manager.write().await;
                    if let Err(e) = crypto_mut.import_group_key(group_id, &encoded_key) {
                        return Err(format!("Failed to load group key: {}", e));
                    }
                } else {
                    return Err("Group encryption key not found".to_string());
                }
            }
        }

        // Ottieni i messaggi crittografati
        let encrypted_messages = self.get_encrypted_group_messages(group_id, limit).await?;
        
        // Decritta ogni messaggio
        let mut decrypted_messages = Vec::new();
        let crypto = self.crypto_manager.read().await;
        
        for msg in encrypted_messages {
            match crypto.decrypt_group_message(group_id, &msg) {
                Ok(content) => {
                    let formatted_msg = format!("[{}] {}: {}", 
                        msg.timestamp.format("%H:%M:%S"),
                        msg.sender_id, // Qui dovremmo convertire in username
                        content
                    );
                    decrypted_messages.push(formatted_msg);
                }
                Err(e) => {
                    warn!("Failed to decrypt message {}: {}", msg.encrypted_content, e);
                    decrypted_messages.push(format!("[{}] [ENCRYPTED MESSAGE]", msg.timestamp.format("%H:%M:%S")));
                }
            }
        }
        
        Ok(decrypted_messages)
    }

    /// Ottieni i messaggi crittografati diretti tra due utenti
    pub async fn get_encrypted_direct_messages(&self, user1_id: Uuid, user2_id: Uuid, limit: i64) -> Result<Vec<EncryptedMessage>, String> {
        match self.db_manager.get_encrypted_direct_messages(user1_id, user2_id, limit).await {
            Ok(messages) => Ok(messages),
            Err(e) => Err(format!("Failed to get direct messages: {}", e)),
        }
    }

    /// Ottieni e decritta i messaggi diretti tra due utenti (per compatibilità con il client)
    pub async fn get_decrypted_direct_messages(&self, user1_id: Uuid, user2_id: Uuid, limit: i64) -> Result<Vec<String>, String> {
        // Carica la chiave per la chat diretta se necessario
        {
            let crypto = self.crypto_manager.read().await;
            if !crypto.has_direct_key(user1_id, user2_id) {
                drop(crypto);
                if let Ok(Some(encoded_key)) = self.db_manager.get_user_encryption_key(user1_id, user2_id).await {
                    let mut crypto_mut = self.crypto_manager.write().await;
                    match BASE64.decode(&encoded_key) {
                        Ok(key_bytes) => {
                            crypto_mut.set_direct_key(user1_id, user2_id, key_bytes);
                        }
                        Err(e) => {
                            return Err(format!("Failed to decode encryption key: {}", e));
                        }
                    }
                } else {
                    return Err("Direct chat encryption key not found".to_string());
                }
            }
        }

        // Ottieni i messaggi crittografati
        let encrypted_messages = self.get_encrypted_direct_messages(user1_id, user2_id, limit).await?;
        
        // Decritta ogni messaggio
        let mut decrypted_messages = Vec::new();
        let crypto = self.crypto_manager.read().await;
        
        for msg in encrypted_messages {
            match crypto.decrypt_direct_message(user1_id, user2_id, &msg) {
                Ok(content) => {
                    let formatted_msg = format!("[{}] {}: {}", 
                        msg.timestamp.format("%H:%M:%S"),
                        msg.sender_id, // Qui dovremmo convertire in username
                        content
                    );
                    decrypted_messages.push(formatted_msg);
                }
                Err(e) => {
                    warn!("Failed to decrypt direct message: {}", e);
                    decrypted_messages.push(format!("[{}] [ENCRYPTED MESSAGE]", msg.timestamp.format("%H:%M:%S")));
                }
            }
        }
        
        Ok(decrypted_messages)
    }

    /// Salva una chiave di crittografia per un gruppo
    pub async fn save_group_encryption_key(&self, group_id: Uuid, encrypted_key: String, created_by: Uuid) -> Result<(), String> {
        match self.db_manager.save_group_encryption_key(group_id, &encrypted_key, created_by).await {
            Ok(()) => Ok(()),
            Err(e) => Err(format!("Failed to save group encryption key: {}", e)),
        }
    }

    /// Ottieni la chiave di crittografia di un gruppo
    pub async fn get_group_encryption_key(&self, group_id: Uuid) -> Result<Option<String>, String> {
        match self.db_manager.get_group_encryption_key(group_id).await {
            Ok(key) => Ok(key),
            Err(e) => Err(format!("Failed to get group encryption key: {}", e)),
        }
    }

    /// Salva una chiave di crittografia per chat diretta
    pub async fn save_user_encryption_key(&self, user1_id: Uuid, user2_id: Uuid, encrypted_key: String) -> Result<(), String> {
        match self.db_manager.save_user_encryption_key(user1_id, user2_id, &encrypted_key).await {
            Ok(()) => Ok(()),
            Err(e) => Err(format!("Failed to save user encryption key: {}", e)),
        }
    }

    /// Ottieni la chiave di crittografia per chat diretta
    pub async fn get_user_encryption_key(&self, user1_id: Uuid, user2_id: Uuid) -> Result<Option<String>, String> {
        match self.db_manager.get_user_encryption_key(user1_id, user2_id).await {
            Ok(key) => Ok(key),
            Err(e) => Err(format!("Failed to get user encryption key: {}", e)),
        }
    }

    // === PERFORMANCE & PERSISTENCE ===
    pub async fn get_performance_metrics(&self) -> (usize, usize, usize) {
        // Conta solo utenti online dal database
        let online_users_count = match self.db_manager.get_online_users().await {
            Ok(online_users) => online_users.len(),
            Err(e) => {
                warn!("Failed to get online users from database: {}", e);
                // Fallback ai dati in memoria (che rappresentano utenti connessi)
                self.users.read().await.len()
            }
        };

        // Recupera statistiche per gruppi e messaggi crittografati dal database
        match self.db_manager.get_database_stats().await {
            Ok((_users_db, groups_db, messages_db, _invites)) => {
                // Usa il conteggio degli utenti online e i dati dal database per gruppi e messaggi crittografati
                (online_users_count, groups_db, messages_db)
            },
            Err(e) => {
                warn!("Failed to get database stats, using memory stats: {}", e);
                // Fallback completo ai dati in memoria se il database fallisce
                let group_count = self.groups.read().await.len();
                let message_count = self.encrypted_messages.read().await.len();
                (online_users_count, group_count, message_count)
            }
        }
    }
}