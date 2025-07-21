use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Rappresenta un utente nel sistema
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub id: Uuid,
    pub username: String,
    pub created_at: DateTime<Utc>,
    pub is_online: bool,
}

/// Rappresenta un gruppo di chat
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Group {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub created_by: Uuid,
    pub created_at: DateTime<Utc>,
    pub members: Vec<Uuid>,
}

/// Rappresenta un messaggio nella chat
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub id: Uuid,
    pub sender_id: Uuid,
    pub group_id: Option<Uuid>, // None per messaggi diretti
    pub content: String,
    pub timestamp: DateTime<Utc>,
    pub message_type: MessageType,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MessageType {
    Text,
    SystemNotification,
    UserJoined,
    UserLeft,
    GroupCreated,
    UserInvited,
}

/// Rappresenta un invito a un gruppo
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupInvite {
    pub id: Uuid,
    pub group_id: Uuid,
    pub inviter_id: Uuid,
    pub invitee_id: Uuid,
    pub created_at: DateTime<Utc>,
    pub status: InviteStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum InviteStatus {
    Pending,
    Accepted,
    Rejected,
    Expired,
}

/// Statistiche di performance del sistema
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceMetrics {
    pub timestamp: DateTime<Utc>,
    pub cpu_usage_percent: f64,
    pub memory_usage_mb: f64,
    pub active_connections: usize,
    pub messages_per_minute: u64,
}
