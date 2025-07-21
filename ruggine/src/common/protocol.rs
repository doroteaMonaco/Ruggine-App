use crate::common::models::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Protocollo di comunicazione client-server
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ClientMessage {
    /// Richiesta di registrazione al primo avvio
    RegisterUser { username: String },
    
    /// Login di un utente esistente
    Login { username: String },
    
    /// Invio di un messaggio
    SendMessage { 
        content: String, 
        group_id: Option<Uuid> 
    },
    
    /// Creazione di un nuovo gruppo
    CreateGroup { 
        name: String, 
        description: Option<String> 
    },
    
    /// Invito di un utente a un gruppo
    InviteToGroup { 
        username: String, 
        group_id: Uuid 
    },
    
    /// Risposta a un invito
    RespondToInvite { 
        invite_id: Uuid, 
        accept: bool 
    },
    
    /// Richiesta lista gruppi dell'utente
    ListMyGroups,
    
    /// Richiesta lista messaggi di un gruppo
    GetGroupMessages { 
        group_id: Uuid, 
        limit: Option<u32> 
    },
    
    /// Disconnessione
    Disconnect,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ServerMessage {
    /// Conferma di registrazione
    RegistrationConfirmed { user: User },
    
    /// Errore di registrazione
    RegistrationFailed { reason: String },
    
    /// Conferma di login
    LoginSuccessful { user: User },
    
    /// Errore di login
    LoginFailed { reason: String },
    
    /// Nuovo messaggio ricevuto
    MessageReceived { message: Message, sender: User },
    
    /// Conferma invio messaggio
    MessageSent { message_id: Uuid },
    
    /// Gruppo creato con successo
    GroupCreated { group: Group },
    
    /// Invito ricevuto
    InviteReceived { invite: GroupInvite, group: Group },
    
    /// Lista dei gruppi dell'utente
    GroupsList { groups: Vec<Group> },
    
    /// Lista messaggi di un gruppo
    GroupMessages { messages: Vec<(Message, User)> },
    
    /// Notifica sistema
    SystemNotification { message: String },
    
    /// Errore generico
    Error { message: String },
    
    /// Conferma disconnessione
    DisconnectConfirmed,
}

/// Configurazione del protocollo
pub const PROTOCOL_VERSION: &str = "1.0";
pub const MAX_MESSAGE_SIZE: usize = 4096;
pub const MAX_USERNAME_LENGTH: usize = 32;
pub const MAX_GROUP_NAME_LENGTH: usize = 64;
