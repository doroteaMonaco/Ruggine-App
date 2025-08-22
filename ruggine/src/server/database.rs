#![allow(dead_code)]

use sqlx::{SqlitePool, Row};
use anyhow::Result;
use chrono::{DateTime, Utc};
use uuid::Uuid;
use log::info;
use crate::common::models::{Group, GroupInvite, InviteStatus, MessageType, User};
use crate::common::crypto::EncryptedMessage;

#[allow(dead_code)]
/// Gestore del database SQLite per l'applicazione Ruggine
pub struct DatabaseManager {
    pool: SqlitePool,
}

impl DatabaseManager {
    /// Crea una nuova connessione al database
    pub async fn new(database_url: &str) -> Result<Self> {
        info!("Connecting to database: {}", database_url);
        
        let pool = SqlitePool::connect(database_url).await?;
        
        info!("Database connection established");
        
        Ok(Self { pool })
    }

    /// Crea un nuovo utente
    pub async fn create_user(&self, user: &User) -> Result<()> {
        sqlx::query(
            "INSERT INTO users (id, username, created_at, is_online) VALUES (?, ?, ?, ?)"
        )
        .bind(user.id.to_string())
        .bind(&user.username)
        .bind(user.created_at.to_rfc3339())
        .bind(user.is_online)
        .execute(&self.pool)
        .await?;

        info!("Created user: {} ({})", user.username, user.id);
        Ok(())
    }

    /// Trova un utente per username
    pub async fn get_user_by_username(&self, username: &str) -> Result<Option<User>> {
        let row = sqlx::query(
            "SELECT id, username, created_at, is_online FROM users WHERE username = ?"
        )
        .bind(username)
        .fetch_optional(&self.pool)
        .await?;

        if let Some(row) = row {
            let user = User {
                id: Uuid::parse_str(&row.get::<String, _>("id"))?,
                username: row.get("username"),
                created_at: DateTime::parse_from_rfc3339(&row.get::<String, _>("created_at"))?.with_timezone(&Utc),
                is_online: row.get("is_online"),
            };
            Ok(Some(user))
        } else {
            Ok(None)
        }
    }

    /// Trova un utente per ID
    pub async fn get_user_by_id(&self, user_id: Uuid) -> Result<Option<User>> {
        let row = sqlx::query(
            "SELECT id, username, created_at, is_online FROM users WHERE id = ?"
        )
        .bind(user_id.to_string())
        .fetch_optional(&self.pool)
        .await?;

        if let Some(row) = row {
            let user = User {
                id: user_id,
                username: row.get("username"),
                created_at: DateTime::parse_from_rfc3339(&row.get::<String, _>("created_at"))?.with_timezone(&Utc),
                is_online: row.get("is_online"),
            };
            Ok(Some(user))
        } else {
            Ok(None)
        }
    }

    /// Aggiorna lo stato online di un utente
    pub async fn update_user_online_status(&self, user_id: Uuid, is_online: bool) -> Result<()> {
        let last_seen = if is_online { None } else { Some(Utc::now().to_rfc3339()) };
        
        sqlx::query(
            "UPDATE users SET is_online = ?, last_seen = ? WHERE id = ?"
        )
        .bind(is_online)
        .bind(last_seen)
        .bind(user_id.to_string())
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Crea un nuovo gruppo
    pub async fn create_group(&self, group: &Group) -> Result<()> {
        let mut tx = self.pool.begin().await?;

        // Inserisci il gruppo
        sqlx::query(
            "INSERT INTO groups (id, name, description, created_by, created_at) VALUES (?, ?, ?, ?, ?)"
        )
        .bind(group.id.to_string())
        .bind(&group.name)
        .bind(&group.description)
        .bind(group.created_by.to_string())
        .bind(group.created_at.to_rfc3339())
        .execute(&mut *tx)
        .await?;

        // Aggiungi il creatore come admin del gruppo
        sqlx::query(
            "INSERT INTO group_members (group_id, user_id, joined_at, role) VALUES (?, ?, ?, 'admin')"
        )
        .bind(group.id.to_string())
        .bind(group.created_by.to_string())
        .bind(group.created_at.to_rfc3339())
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;
        info!("Created group: {} ({})", group.name, group.id);
        Ok(())
    }

    /// Ottieni i gruppi di un utente
    pub async fn get_user_groups(&self, user_id: Uuid) -> Result<Vec<Group>> {
        let rows = sqlx::query(
            r#"
            SELECT g.id, g.name, g.description, g.created_by, g.created_at
            FROM groups g
            JOIN group_members gm ON g.id = gm.group_id
            WHERE gm.user_id = ? AND g.is_active = true
            ORDER BY g.name
            "#
        )
        .bind(user_id.to_string())
        .fetch_all(&self.pool)
        .await?;

        let mut groups = Vec::new();
        for row in rows {
            let group_id = Uuid::parse_str(&row.get::<String, _>("id"))?;
            
            // Ottieni i membri del gruppo
            let members = self.get_group_members(group_id).await?;
            
            let group = Group {
                id: group_id,
                name: row.get("name"),
                description: row.get("description"),
                created_by: Uuid::parse_str(&row.get::<String, _>("created_by"))?,
                created_at: DateTime::parse_from_rfc3339(&row.get::<String, _>("created_at"))?.with_timezone(&Utc),
                members,
            };
            groups.push(group);
        }

        Ok(groups)
    }

    /// Ottieni un gruppo per ID
    pub async fn get_group_by_id(&self, group_id: Uuid) -> Result<Option<Group>> {
        let row = sqlx::query(
            "SELECT id, name, description, created_by, created_at FROM groups WHERE id = ? AND is_active = true"
        )
        .bind(group_id.to_string())
        .fetch_optional(&self.pool)
        .await?;

        if let Some(row) = row {
            let members = self.get_group_members(group_id).await?;
            
            let group = Group {
                id: group_id,
                name: row.get("name"),
                description: row.get("description"),
                created_by: Uuid::parse_str(&row.get::<String, _>("created_by"))?,
                created_at: DateTime::parse_from_rfc3339(&row.get::<String, _>("created_at"))?.with_timezone(&Utc),
                members,
            };
            Ok(Some(group))
        } else {
            Ok(None)
        }
    }

    /// Ottieni un gruppo per nome
    pub async fn get_group_by_name(&self, group_name: &str) -> Result<Option<Group>> {
        let row = sqlx::query(
            r#"
            SELECT g.id, g.name, g.description, g.created_by, g.created_at
            FROM groups g
            WHERE g.name = ?
            "#
        )
        .bind(group_name)
        .fetch_optional(&self.pool)
        .await?;

        if let Some(row) = row {
            let group_id = Uuid::parse_str(&row.get::<String, _>("id"))?;
            let members = self.get_group_members(group_id).await?;
            
            let group = Group {
                id: group_id,
                name: row.get("name"),
                description: row.get("description"),
                created_by: Uuid::parse_str(&row.get::<String, _>("created_by"))?,
                created_at: DateTime::parse_from_rfc3339(&row.get::<String, _>("created_at"))?.with_timezone(&Utc),
                members,
            };
            Ok(Some(group))
        } else {
            Ok(None)
        }
    }

    /// Ottieni i membri di un gruppo
    pub async fn get_group_members(&self, group_id: Uuid) -> Result<Vec<Uuid>> {
        let rows = sqlx::query(
            "SELECT user_id FROM group_members WHERE group_id = ?"
        )
        .bind(group_id.to_string())
        .fetch_all(&self.pool)
        .await?;

        let members = rows.iter()
            .map(|row| Uuid::parse_str(&row.get::<String, _>("user_id")))
            .collect::<Result<Vec<_>, _>>()?;

        Ok(members)
    }

    /// Salva un messaggio crittografato
    pub async fn save_encrypted_message(&self, encrypted_msg: &EncryptedMessage) -> Result<Uuid> {
        let message_id = Uuid::new_v4();
        
        sqlx::query(
            r#"
            INSERT INTO encrypted_messages (id, sender_id, group_id, receiver_id, encrypted_content, nonce, timestamp, message_type)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?)
            "#
        )
        .bind(message_id.to_string())
        .bind(encrypted_msg.sender_id.to_string())
        .bind(encrypted_msg.group_id.map(|id| id.to_string()))
        .bind(encrypted_msg.receiver_id.map(|id| id.to_string()))
        .bind(&encrypted_msg.encrypted_content)
        .bind(&encrypted_msg.nonce)
        .bind(encrypted_msg.timestamp.to_rfc3339())
        .bind(format!("{:?}", encrypted_msg.message_type))
        .execute(&self.pool)
        .await?;

        info!("Encrypted message saved from user {}", encrypted_msg.sender_id);
        Ok(message_id)
    }

    /// Ottieni i messaggi crittografati di un gruppo
    pub async fn get_encrypted_group_messages(&self, group_id: Uuid, limit: i64) -> Result<Vec<EncryptedMessage>> {
        let rows = sqlx::query(
            r#"
            SELECT em.id, em.sender_id, em.group_id, em.receiver_id, em.encrypted_content, 
                   em.nonce, em.timestamp, em.message_type, u.username as sender_username
            FROM encrypted_messages em
            JOIN users u ON em.sender_id = u.id
            WHERE em.group_id = ?
            ORDER BY em.timestamp ASC
            LIMIT ?
            "#
        )
        .bind(group_id.to_string())
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        let mut messages = Vec::new();
        for row in rows {
            let message = EncryptedMessage {
                sender_id: Uuid::parse_str(&row.get::<String, _>("sender_id"))?,
                sender_username: row.get("sender_username"),
                group_id: Some(group_id),
                receiver_id: row.get::<Option<String>, _>("receiver_id")
                    .map(|id| Uuid::parse_str(&id)).transpose()?,
                encrypted_content: row.get("encrypted_content"),
                nonce: row.get("nonce"),
                timestamp: DateTime::parse_from_rfc3339(&row.get::<String, _>("timestamp"))?.with_timezone(&Utc),
                message_type: match row.get::<String, _>("message_type").as_str() {
                    "Text" => MessageType::Text,
                    "File" => MessageType::File,
                    "Image" => MessageType::Image,
                    _ => MessageType::Text,
                },
            };
            messages.push(message);
        }

        // Messaggi già in ordine cronologico corretto (più vecchi prima)
        Ok(messages)
    }

    /// Ottieni i messaggi crittografati diretti tra due utenti
    pub async fn get_encrypted_direct_messages(&self, user1_id: Uuid, user2_id: Uuid, limit: i64) -> Result<Vec<EncryptedMessage>> {
        let rows = sqlx::query(
            r#"
            SELECT em.sender_id, em.group_id, em.receiver_id, em.encrypted_content, 
                   em.nonce, em.timestamp, em.message_type, u.username as sender_username
            FROM encrypted_messages em
            JOIN users u ON em.sender_id = u.id
            WHERE em.group_id IS NULL 
              AND ((em.sender_id = ? AND em.receiver_id = ?) OR (em.sender_id = ? AND em.receiver_id = ?))
            ORDER BY em.timestamp ASC
            LIMIT ?
            "#
        )
        .bind(user1_id.to_string())
        .bind(user2_id.to_string())
        .bind(user2_id.to_string())
        .bind(user1_id.to_string())
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        let mut messages = Vec::new();
        for row in rows {
            let message = EncryptedMessage {
                sender_id: Uuid::parse_str(&row.get::<String, _>("sender_id"))?,
                sender_username: row.get("sender_username"),
                group_id: None,
                receiver_id: row.get::<Option<String>, _>("receiver_id")
                    .map(|s| Uuid::parse_str(&s))
                    .transpose()?,
                encrypted_content: row.get("encrypted_content"),
                nonce: row.get("nonce"),
                timestamp: DateTime::parse_from_rfc3339(&row.get::<String, _>("timestamp"))?.with_timezone(&Utc),
                message_type: match row.get::<String, _>("message_type").as_str() {
                    "Text" => MessageType::Text,
                    "File" => MessageType::File,
                    "Image" => MessageType::Image,
                    _ => MessageType::Text,
                },
            };
            messages.push(message);
        }

        // Messaggi già in ordine cronologico corretto (più vecchi prima)
        Ok(messages)
    }

    /// Crea un invito a un gruppo
    pub async fn create_group_invite(&self, invite: &GroupInvite) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO group_invites (id, group_id, inviter_id, invitee_id, created_at, expires_at, status, responded_at)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?)
            "#
        )
        .bind(invite.id.to_string())
        .bind(invite.group_id.to_string())
        .bind(invite.inviter_id.to_string())
        .bind(invite.invitee_id.to_string())
        .bind(invite.created_at.to_rfc3339())
        .bind(invite.expires_at.map(|dt| dt.to_rfc3339()))
        .bind(format!("{:?}", invite.status))
        .bind(invite.responded_at.map(|dt| dt.to_rfc3339()))
        .execute(&self.pool)
        .await?;

        info!("Created group invite: {}", invite.id);
        Ok(())
    }

    /// Ottieni gli inviti pendenti per un utente
    pub async fn get_pending_invites(&self, user_id: Uuid) -> Result<Vec<GroupInvite>> {
        let rows = sqlx::query(
            r#"
            SELECT id, group_id, inviter_id, invitee_id, created_at, status, expires_at, responded_at
            FROM group_invites
            WHERE invitee_id = ? AND status = 'Pending'
            ORDER BY created_at DESC
            "#
        )
        .bind(user_id.to_string())
        .fetch_all(&self.pool)
        .await?;

        let mut invites = Vec::new();
        for row in rows {
            let expires_at = row.get::<Option<String>, _>("expires_at")
                .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
                .map(|dt| dt.with_timezone(&Utc));
                
            let responded_at = row.get::<Option<String>, _>("responded_at")
                .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
                .map(|dt| dt.with_timezone(&Utc));
                
            let invite = GroupInvite {
                id: Uuid::parse_str(&row.get::<String, _>("id"))?,
                group_id: Uuid::parse_str(&row.get::<String, _>("group_id"))?,
                inviter_id: Uuid::parse_str(&row.get::<String, _>("inviter_id"))?,
                invitee_id: Uuid::parse_str(&row.get::<String, _>("invitee_id"))?,
                created_at: DateTime::parse_from_rfc3339(&row.get::<String, _>("created_at"))?.with_timezone(&Utc),
                expires_at,
                status: InviteStatus::Pending,
                responded_at,
            };
            invites.push(invite);
        }

        Ok(invites)
    }

    /// Accetta un invito a un gruppo
    pub async fn accept_group_invite(&self, invite_id: Uuid) -> Result<()> {
        let mut tx = self.pool.begin().await?;

        // Ottieni i dettagli dell'invito
        let invite_row = sqlx::query(
            "SELECT group_id, invitee_id FROM group_invites WHERE id = ? AND status = 'Pending'"
        )
        .bind(invite_id.to_string())
        .fetch_one(&mut *tx)
        .await?;

        let group_id = invite_row.get::<String, _>("group_id");
        let user_id = invite_row.get::<String, _>("invitee_id");

        // Aggiorna lo status dell'invito
        sqlx::query(
            "UPDATE group_invites SET status = 'Accepted', responded_at = ? WHERE id = ?"
        )
        .bind(Utc::now().to_rfc3339())
        .bind(invite_id.to_string())
        .execute(&mut *tx)
        .await?;

        // Aggiungi l'utente al gruppo
        sqlx::query(
            "INSERT INTO group_members (group_id, user_id, joined_at, role) VALUES (?, ?, ?, 'member')"
        )
        .bind(&group_id)
        .bind(&user_id)
        .bind(Utc::now().to_rfc3339())
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;
        info!("User {} joined group {} via invite {}", user_id, group_id, invite_id);
        Ok(())
    }

    /// Cleanup dei dati vecchi (per mantenere prestazioni)
    pub async fn cleanup_old_data(&self, days_to_keep: i64) -> Result<()> {
        let cutoff_date = (Utc::now() - chrono::Duration::days(days_to_keep)).to_rfc3339();

        // Rimuovi inviti scaduti
        let deleted_invites = sqlx::query(
            "DELETE FROM group_invites WHERE created_at < ? AND status IN ('Rejected', 'Expired')"
        )
        .bind(&cutoff_date)
        .execute(&self.pool)
        .await?
        .rows_affected();

        info!("Cleanup completed: {} invites", deleted_invites);
        Ok(())
    }

    /// Salva una chiave di crittografia per un gruppo
    pub async fn save_group_encryption_key(&self, group_id: Uuid, encrypted_key: &str, created_by: Uuid) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO group_encryption_keys (group_id, encrypted_key, created_by, created_at)
            VALUES (?, ?, ?, ?)
            "#
        )
        .bind(group_id.to_string())
        .bind(encrypted_key)
        .bind(created_by.to_string())
        .bind(Utc::now().to_rfc3339())
        .execute(&self.pool)
        .await?;

        info!("Encryption key saved for group {}", group_id);
        Ok(())
    }

    /// Ottieni la chiave di crittografia attiva di un gruppo
    pub async fn get_group_encryption_key(&self, group_id: Uuid) -> Result<Option<String>> {
        let row = sqlx::query(
            "SELECT encrypted_key FROM group_encryption_keys WHERE group_id = ? AND is_active = true ORDER BY created_at DESC LIMIT 1"
        )
        .bind(group_id.to_string())
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|r| r.get("encrypted_key")))
    }

    /// Salva una chiave di crittografia per chat diretta
    pub async fn save_user_encryption_key(&self, user1_id: Uuid, user2_id: Uuid, encrypted_key: &str) -> Result<()> {
        // Assicurati che user1_id < user2_id per consistenza
        let (u1, u2) = if user1_id < user2_id { (user1_id, user2_id) } else { (user2_id, user1_id) };
        
        sqlx::query(
            r#"
            INSERT OR REPLACE INTO user_encryption_keys (user1_id, user2_id, encrypted_key, created_at)
            VALUES (?, ?, ?, ?)
            "#
        )
        .bind(u1.to_string())
        .bind(u2.to_string())
        .bind(encrypted_key)
        .bind(Utc::now().to_rfc3339())
        .execute(&self.pool)
        .await?;

        info!("Encryption key saved for users {} and {}", u1, u2);
        Ok(())
    }

    /// Ottieni la chiave di crittografia per chat diretta
    pub async fn get_user_encryption_key(&self, user1_id: Uuid, user2_id: Uuid) -> Result<Option<String>> {
        let (u1, u2) = if user1_id < user2_id { (user1_id, user2_id) } else { (user2_id, user1_id) };
        
        let row = sqlx::query(
            "SELECT encrypted_key FROM user_encryption_keys WHERE user1_id = ? AND user2_id = ? AND is_active = true"
        )
        .bind(u1.to_string())
        .bind(u2.to_string())
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|r| r.get("encrypted_key")))
    }

    /// Ottieni utenti online
    pub async fn get_online_users(&self) -> Result<Vec<User>> {
        let rows = sqlx::query(
            "SELECT id, username, created_at, is_online, last_seen FROM users WHERE is_online = true ORDER BY username"
        )
        .fetch_all(&self.pool)
        .await?;

        let mut users = Vec::new();
        for row in rows {
            let user = User {
                id: Uuid::parse_str(&row.get::<String, _>("id"))?,
                username: row.get("username"),
                created_at: DateTime::parse_from_rfc3339(&row.get::<String, _>("created_at"))?.with_timezone(&Utc),
                is_online: row.get("is_online"),
            };
            users.push(user);
        }

        Ok(users)
    }

    /// Ottieni tutti gli utenti
    pub async fn get_all_users(&self) -> Result<Vec<User>> {
        let rows = sqlx::query(
            "SELECT id, username, created_at, is_online, last_seen FROM users ORDER BY username"
        )
        .fetch_all(&self.pool)
        .await?;

        let mut users = Vec::new();
        for row in rows {
            let user = User {
                id: Uuid::parse_str(&row.get::<String, _>("id"))?,
                username: row.get("username"),
                created_at: DateTime::parse_from_rfc3339(&row.get::<String, _>("created_at"))?.with_timezone(&Utc),
                is_online: row.get("is_online"),
            };
            users.push(user);
        }

        Ok(users)
    }

    /// Aggiunge un utente a un gruppo
    pub async fn add_user_to_group(&self, group_id: Uuid, user_id: Uuid) -> Result<()> {
        sqlx::query("INSERT INTO group_members (group_id, user_id, joined_at) VALUES (?, ?, ?)")
            .bind(group_id.to_string())
            .bind(user_id.to_string())
            .bind(chrono::Utc::now().to_rfc3339())
            .execute(&self.pool)
            .await?;
        
        info!("Added user {} to group {}", user_id, group_id);
        Ok(())
    }

    /// Rimuove un utente da un gruppo
    pub async fn remove_user_from_group(&self, group_id: Uuid, user_id: Uuid) -> Result<()> {
        sqlx::query("DELETE FROM group_members WHERE group_id = ? AND user_id = ?")
            .bind(group_id.to_string())
            .bind(user_id.to_string())
            .execute(&self.pool)
            .await?;
        
        info!("Removed user {} from group {}", user_id, group_id);
        Ok(())
    }
    pub async fn get_database_stats(&self) -> Result<(usize, usize, usize, usize)> {
        // Conta utenti
        let users_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users")
            .fetch_one(&self.pool)
            .await?;

        // Conta gruppi attivi
        let groups_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM groups WHERE is_active = true")
            .fetch_one(&self.pool)
            .await?;

        // Conta messaggi crittografati totali
        let messages_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM encrypted_messages")
            .fetch_one(&self.pool)
            .await?;

        // Conta inviti pendenti
        let pending_invites: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM group_invites WHERE status = 'Pending'")
            .fetch_one(&self.pool)
            .await?;

        Ok((users_count as usize, groups_count as usize, messages_count as usize, pending_invites as usize))
    }

    /// Controlla se un utente è membro di un gruppo
    pub async fn is_user_in_group(&self, user_id: Uuid, group_id: Uuid) -> Result<bool> {
        let count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM group_members WHERE user_id = ? AND group_id = ?"
        )
        .bind(user_id.to_string())
        .bind(group_id.to_string())
        .fetch_one(&self.pool)
        .await?;

        Ok(count > 0)
    }

    /// Ottieni gli inviti pendenti per un utente (alias per get_pending_invites)
    pub async fn get_user_pending_invites(&self, user_id: Uuid) -> Result<Vec<GroupInvite>> {
        self.get_pending_invites(user_id).await
    }

    /// Rifiuta un invito di gruppo
    pub async fn reject_group_invite(&self, invite_id: Uuid) -> Result<()> {
        sqlx::query(
            "UPDATE group_invites SET status = 'rejected' WHERE id = ?"
        )
        .bind(invite_id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Reset all users to offline status (useful at server startup)
    pub async fn reset_all_users_offline(&self) -> Result<()> {
        sqlx::query("UPDATE users SET is_online = false, last_seen = ?")
            .bind(chrono::Utc::now().to_rfc3339())
            .execute(&self.pool)
            .await?;
        
        info!("Reset all users to offline status");
        Ok(())
    }

    /// Check if a user is an admin of a specific group
    pub async fn is_user_group_admin(&self, user_id: Uuid, group_id: Uuid) -> Result<bool> {
        let result: Option<String> = sqlx::query_scalar(
            "SELECT role FROM group_members WHERE user_id = ? AND group_id = ?"
        )
        .bind(user_id.to_string())
        .bind(group_id.to_string())
        .fetch_optional(&self.pool)
        .await?;

        match result {
            Some(role) => Ok(role == "admin"),
            None => Ok(false), // User is not a member of the group
        }
    }

    /// Delete all messages from a group chat
    pub async fn delete_group_messages(&self, group_id: Uuid, user_id: Uuid) -> Result<u64> {
        // Verifica che l'utente sia membro del gruppo
        if !self.is_user_in_group(user_id, group_id).await? {
            return Err(anyhow::anyhow!("User is not a member of this group"));
        }

        let deleted_count = sqlx::query(
            "DELETE FROM encrypted_messages WHERE group_id = ?"
        )
        .bind(group_id.to_string())
        .execute(&self.pool)
        .await?
        .rows_affected();

        info!("Deleted {} group messages from group {} by user {}", deleted_count, group_id, user_id);
        Ok(deleted_count)
    }

    /// Delete all messages from a private chat between two users
    pub async fn delete_private_messages(&self, user1_id: Uuid, user2_id: Uuid) -> Result<u64> {
        let deleted_count = sqlx::query(
            r#"
            DELETE FROM encrypted_messages 
            WHERE group_id IS NULL 
              AND ((sender_id = ? AND receiver_id = ?) OR (sender_id = ? AND receiver_id = ?))
            "#
        )
        .bind(user1_id.to_string())
        .bind(user2_id.to_string())
        .bind(user2_id.to_string())
        .bind(user1_id.to_string())
        .execute(&self.pool)
        .await?
        .rows_affected();

        info!("Deleted {} private messages between users {} and {}", deleted_count, user1_id, user2_id);
        Ok(deleted_count)
    }

    /// Debug method to get all groups
    pub async fn debug_get_all_groups(&self) -> Result<Vec<(String, String)>> {
        let groups = sqlx::query_as::<_, (String, String)>(
            "SELECT id, name FROM groups"
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(groups)
    }

    /// Debug method to get all members of a group
    pub async fn debug_get_group_members(&self, group_id: Uuid) -> Result<Vec<String>> {
        let members = sqlx::query_scalar::<_, String>(
            "SELECT u.username FROM group_members gm JOIN users u ON gm.user_id = u.id WHERE gm.group_id = ?"
        )
        .bind(group_id.to_string())
        .fetch_all(&self.pool)
        .await?;
        Ok(members)
    }

    /// Debug method to check membership directly
    pub async fn debug_check_membership_direct(&self, user_id: Uuid, group_id: Uuid) -> Result<i64> {
        let count = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM group_members WHERE user_id = ? AND group_id = ?"
        )
        .bind(user_id.to_string())
        .bind(group_id.to_string())
        .fetch_one(&self.pool)
        .await?;
        Ok(count)
    }

    // ======================= FRIENDSHIP SYSTEM =======================

    /// Invia una richiesta di amicizia
    pub async fn send_friend_request(&self, sender_id: Uuid, receiver_id: Uuid, message: Option<String>) -> Result<()> {
        // Verifica che non siano già amici
        if self.are_friends(sender_id, receiver_id).await? {
            return Err(anyhow::anyhow!("Users are already friends"));
        }

        // Verifica che non ci sia già una richiesta pendente
        let existing = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM friend_requests WHERE sender_id = ? AND receiver_id = ? AND status = 'pending'"
        )
        .bind(sender_id.to_string())
        .bind(receiver_id.to_string())
        .fetch_one(&self.pool)
        .await?;

        if existing > 0 {
            return Err(anyhow::anyhow!("Friend request already pending"));
        }

        sqlx::query(
            "INSERT INTO friend_requests (sender_id, receiver_id, message, status) VALUES (?, ?, ?, 'pending')"
        )
        .bind(sender_id.to_string())
        .bind(receiver_id.to_string())
        .bind(message)
        .execute(&self.pool)
        .await?;

        info!("Friend request sent from {} to {}", sender_id, receiver_id);
        Ok(())
    }

    /// Accetta una richiesta di amicizia
    pub async fn accept_friend_request(&self, sender_id: Uuid, receiver_id: Uuid) -> Result<()> {
        let mut tx = self.pool.begin().await?;

        // Aggiorna lo status della richiesta
        let result = sqlx::query(
            "UPDATE friend_requests SET status = 'accepted', updated_at = CURRENT_TIMESTAMP 
             WHERE sender_id = ? AND receiver_id = ? AND status = 'pending'"
        )
        .bind(sender_id.to_string())
        .bind(receiver_id.to_string())
        .execute(&mut *tx)
        .await?;

        if result.rows_affected() == 0 {
            return Err(anyhow::anyhow!("No pending friend request found"));
        }

        // Crea l'amicizia (ordine canonico: user1 < user2)
        let (user1, user2) = if sender_id < receiver_id {
            (sender_id, receiver_id)
        } else {
            (receiver_id, sender_id)
        };

        sqlx::query(
            "INSERT OR IGNORE INTO friendships (user1_id, user2_id) VALUES (?, ?)"
        )
        .bind(user1.to_string())
        .bind(user2.to_string())
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;
        info!("Friend request accepted: {} and {} are now friends", sender_id, receiver_id);
        Ok(())
    }

    /// Rifiuta una richiesta di amicizia
    pub async fn reject_friend_request(&self, sender_id: Uuid, receiver_id: Uuid) -> Result<()> {
        let result = sqlx::query(
            "UPDATE friend_requests SET status = 'rejected', updated_at = CURRENT_TIMESTAMP 
             WHERE sender_id = ? AND receiver_id = ? AND status = 'pending'"
        )
        .bind(sender_id.to_string())
        .bind(receiver_id.to_string())
        .execute(&self.pool)
        .await?;

        if result.rows_affected() == 0 {
            return Err(anyhow::anyhow!("No pending friend request found"));
        }

        info!("Friend request rejected: {} -> {}", sender_id, receiver_id);
        Ok(())
    }

    /// Verifica se due utenti sono amici
    pub async fn are_friends(&self, user1_id: Uuid, user2_id: Uuid) -> Result<bool> {
        let (u1, u2) = if user1_id < user2_id {
            (user1_id, user2_id)
        } else {
            (user2_id, user1_id)
        };

        let count = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM friendships WHERE user1_id = ? AND user2_id = ?"
        )
        .bind(u1.to_string())
        .bind(u2.to_string())
        .fetch_one(&self.pool)
        .await?;

        Ok(count > 0)
    }

    /// Ottieni tutti gli amici di un utente
    pub async fn get_user_friends(&self, user_id: Uuid) -> Result<Vec<User>> {
        let rows = sqlx::query(
            r#"
            SELECT u.id, u.username, u.created_at, u.is_online
            FROM users u
            JOIN user_friendships uf ON u.id = uf.friend_id
            WHERE uf.user_id = ?
            ORDER BY u.username
            "#
        )
        .bind(user_id.to_string())
        .fetch_all(&self.pool)
        .await?;

        let mut friends = Vec::new();
        for row in rows {
            let friend = User {
                id: Uuid::parse_str(&row.get::<String, _>("id"))?,
                username: row.get("username"),
                created_at: DateTime::parse_from_rfc3339(&row.get::<String, _>("created_at"))?.with_timezone(&Utc),
                is_online: row.get("is_online"),
            };
            friends.push(friend);
        }

        Ok(friends)
    }

    /// Ottieni le richieste di amicizia ricevute da un utente
    pub async fn get_received_friend_requests(&self, user_id: Uuid) -> Result<Vec<(User, String, DateTime<Utc>)>> {
        let rows = sqlx::query(
            r#"
            SELECT u.id, u.username, u.created_at, u.is_online, 
                   fr.message, fr.created_at as request_created_at
            FROM friend_requests fr
            JOIN users u ON fr.sender_id = u.id
            WHERE fr.receiver_id = ? AND fr.status = 'pending'
            ORDER BY fr.created_at ASC
            "#
        )
        .bind(user_id.to_string())
        .fetch_all(&self.pool)
        .await?;

        let mut requests = Vec::new();
        
        for row in rows {
            let sender = User {
                id: Uuid::parse_str(&row.get::<String, _>("id"))?,
                username: row.get("username"),
                created_at: DateTime::parse_from_rfc3339(&row.get::<String, _>("created_at"))?.with_timezone(&Utc),
                is_online: row.get("is_online"),
            };
            let message = row.get::<Option<String>, _>("message").unwrap_or_default();
            
            // Gestisci il timestamp SQLite in modo più robusto
            let request_timestamp = row.get::<String, _>("request_created_at");
            let request_date = if let Ok(dt) = DateTime::parse_from_rfc3339(&request_timestamp) {
                dt.with_timezone(&Utc)
            } else {
                // Fallback: usa il timestamp corrente se il parsing fallisce
                Utc::now()
            };
            
            requests.push((sender, message, request_date));
        }

        Ok(requests)
    }

    /// Ottieni le richieste di amicizia inviate da un utente
    pub async fn get_sent_friend_requests(&self, user_id: Uuid) -> Result<Vec<(User, String, DateTime<Utc>)>> {
        let rows = sqlx::query(
            r#"
            SELECT u.id, u.username, u.created_at, u.is_online, 
                   fr.message, fr.created_at as request_created_at
            FROM friend_requests fr
            JOIN users u ON fr.receiver_id = u.id
            WHERE fr.sender_id = ? AND fr.status = 'pending'
            ORDER BY fr.created_at ASC
            "#
        )
        .bind(user_id.to_string())
        .fetch_all(&self.pool)
        .await?;

        let mut requests = Vec::new();
        for row in rows {
            let receiver = User {
                id: Uuid::parse_str(&row.get::<String, _>("id"))?,
                username: row.get("username"),
                created_at: DateTime::parse_from_rfc3339(&row.get::<String, _>("created_at"))?.with_timezone(&Utc),
                is_online: row.get("is_online"),
            };
            let message = row.get::<Option<String>, _>("message").unwrap_or_default();
            
            // Gestisci il timestamp SQLite in modo più robusto
            let request_timestamp = row.get::<String, _>("request_created_at");
            let request_date = if let Ok(dt) = DateTime::parse_from_rfc3339(&request_timestamp) {
                dt.with_timezone(&Utc)
            } else {
                // Fallback: usa il timestamp corrente se il parsing fallisce
                Utc::now()
            };
            
            requests.push((receiver, message, request_date));
        }

        Ok(requests)
    }

    /// Rimuovi un'amicizia
    pub async fn remove_friendship(&self, user1_id: Uuid, user2_id: Uuid) -> Result<()> {
        let (u1, u2) = if user1_id < user2_id {
            (user1_id, user2_id)
        } else {
            (user2_id, user1_id)
        };

        sqlx::query(
            "DELETE FROM friendships WHERE user1_id = ? AND user2_id = ?"
        )
        .bind(u1.to_string())
        .bind(u2.to_string())
        .execute(&self.pool)
        .await?;

        info!("Friendship removed between {} and {}", user1_id, user2_id);
        Ok(())
    }

    /// Esegue le migrazioni del database per il sistema di amicizie
    pub async fn run_friendship_migrations(&self) -> Result<()> {
        // Prima rimuovi le tabelle esistenti se hanno foreign key errate
        sqlx::query("DROP TABLE IF EXISTS friend_requests").execute(&self.pool).await?;
        sqlx::query("DROP TABLE IF EXISTS friendships").execute(&self.pool).await?;
        sqlx::query("DROP VIEW IF EXISTS user_friendships").execute(&self.pool).await?;
        
        // Crea le tabelle direttamente invece di usare include_str
        
        // Tabella per le richieste di amicizia
        sqlx::query(r#"
            CREATE TABLE IF NOT EXISTS friend_requests (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                sender_id TEXT NOT NULL,
                receiver_id TEXT NOT NULL,
                status TEXT NOT NULL DEFAULT 'pending',
                message TEXT,
                created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                
                FOREIGN KEY (sender_id) REFERENCES users(id) ON DELETE CASCADE,
                FOREIGN KEY (receiver_id) REFERENCES users(id) ON DELETE CASCADE,
                UNIQUE(sender_id, receiver_id),
                CHECK (sender_id != receiver_id),
                CHECK (status IN ('pending', 'accepted', 'rejected'))
            )
        "#).execute(&self.pool).await?;

        // Tabella per le amicizie confermate
        sqlx::query(r#"
            CREATE TABLE IF NOT EXISTS friendships (
                user1_id TEXT NOT NULL,
                user2_id TEXT NOT NULL,
                created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                
                PRIMARY KEY (user1_id, user2_id),
                FOREIGN KEY (user1_id) REFERENCES users(id) ON DELETE CASCADE,
                FOREIGN KEY (user2_id) REFERENCES users(id) ON DELETE CASCADE,
                CHECK (user1_id < user2_id)
            )
        "#).execute(&self.pool).await?;

        // Indici per performance
        sqlx::query("CREATE INDEX IF NOT EXISTS idx_friend_requests_sender ON friend_requests(sender_id)")
            .execute(&self.pool).await?;
        
        sqlx::query("CREATE INDEX IF NOT EXISTS idx_friend_requests_receiver ON friend_requests(receiver_id)")
            .execute(&self.pool).await?;
        
        sqlx::query("CREATE INDEX IF NOT EXISTS idx_friend_requests_status ON friend_requests(status)")
            .execute(&self.pool).await?;
        
        sqlx::query("CREATE INDEX IF NOT EXISTS idx_friendships_user1 ON friendships(user1_id)")
            .execute(&self.pool).await?;
        
        sqlx::query("CREATE INDEX IF NOT EXISTS idx_friendships_user2 ON friendships(user2_id)")
            .execute(&self.pool).await?;

        // Trigger per aggiornare updated_at
        sqlx::query(r#"
            CREATE TRIGGER IF NOT EXISTS update_friend_requests_timestamp 
            AFTER UPDATE ON friend_requests
            BEGIN
                UPDATE friend_requests SET updated_at = CURRENT_TIMESTAMP WHERE id = NEW.id;
            END
        "#).execute(&self.pool).await?;

        // View per semplificare le query di amicizia (bidirezionale)
        sqlx::query(r#"
            CREATE VIEW IF NOT EXISTS user_friendships AS
            SELECT 
                user1_id as user_id, 
                user2_id as friend_id, 
                created_at
            FROM friendships
            UNION ALL
            SELECT 
                user2_id as user_id, 
                user1_id as friend_id, 
                created_at
            FROM friendships
        "#).execute(&self.pool).await?;

        info!("Friendship system migrations completed successfully");
        Ok(())
    }

}
