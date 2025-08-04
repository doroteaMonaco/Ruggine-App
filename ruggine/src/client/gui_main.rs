use iced::{
    widget::{button, column, container, row, text, text_input, radio, scrollable},
    Application, Command, Element, Length, Settings, Theme, Font,
};
use log::error;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::net::TcpStream;
use tokio::io::{AsyncWriteExt, AsyncBufReadExt, BufWriter, BufReader};
use sqlx::{sqlite::SqlitePool, Row};

use ruggine::client::config::ClientConfig;

// Font per emoji
const EMOJI_FONT: Font = Font::with_name("Segoe UI Emoji");
// Font grassetto per i titoli
const BOLD_FONT: Font = Font {
    family: iced::font::Family::SansSerif,
    weight: iced::font::Weight::Bold,
    ..Font::DEFAULT
};

// Risultato dell'inizializzazione database
#[derive(Debug, Clone)]
pub enum DatabaseInitializationResult {
    Success(ScalableDeletedChatsManager),
    Error(String),
}

// Struttura scalabile per gestione persistenza chat eliminate
#[derive(Debug, Clone)]
struct ScalableDeletedChatsManager {
    db_pool: Option<SqlitePool>,
    memory_cache: std::collections::HashMap<String, std::time::SystemTime>,
    cache_size_limit: usize,
}

impl ScalableDeletedChatsManager {
    /// Cache con limite di dimensione (default: 1000 entries)
    fn new() -> Self {
        Self {
            db_pool: None,
            memory_cache: std::collections::HashMap::new(),
            cache_size_limit: 1000,
        }
    }
    
    /// Inizializza il database SQLite per la persistenza
    async fn initialize_database(&mut self) -> Result<(), sqlx::Error> {
        let database_url = "sqlite:data/ruggine.db"; // Usa lo stesso database dell'app
        
        // Crea la directory se non esiste
        if let Some(parent) = std::path::Path::new("data/ruggine.db").parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        
        println!("🔗 Connecting to database: {}", database_url);
        let pool = SqlitePool::connect(database_url).await?;
        
        // Crea la tabella se non esiste
        println!("🏗️ Creating deleted_chats table if not exists...");
        sqlx::query(r#"
            CREATE TABLE IF NOT EXISTS deleted_chats (
                username TEXT PRIMARY KEY,
                deleted_at INTEGER NOT NULL,
                created_at INTEGER DEFAULT (strftime('%s', 'now'))
            );
            CREATE INDEX IF NOT EXISTS idx_deleted_at ON deleted_chats(deleted_at);
            CREATE INDEX IF NOT EXISTS idx_created_at ON deleted_chats(created_at);
        "#)
        .execute(&pool)
        .await?;
        
        self.db_pool = Some(pool);
        println!("✅ Scalable deleted chats database initialized successfully!");
        Ok(())
    }
    
    /// Ottiene il timestamp di eliminazione per filtrare i messaggi (restituisce None se non eliminata)
    async fn get_deletion_timestamp(&mut self, username: &str) -> Result<Option<std::time::SystemTime>, Box<dyn std::error::Error + Send + Sync>> {
        // Prima controlla la cache
        if let Some(timestamp) = self.memory_cache.get(username) {
            println!("📋 Cache hit for user '{}': timestamp = {:?}", username, timestamp);
            return Ok(Some(*timestamp));
        }
        
        println!("📭 Cache miss for user '{}', checking database...", username);
        
        // Se non in cache e abbiamo il database, carica dal DB
        if let Some(pool) = &self.db_pool {
            println!("🔗 Database pool available, querying for user '{}'", username);
            match sqlx::query("SELECT deleted_at FROM deleted_chats WHERE username = ?")
                .bind(username)
                .fetch_optional(pool)
                .await
            {
                Ok(Some(row)) => {
                    let timestamp_secs: i64 = row.get("deleted_at");
                    println!("📅 Found database entry for user '{}': timestamp_secs = {}", username, timestamp_secs);
                    if let Some(system_time) = std::time::UNIX_EPOCH.checked_add(std::time::Duration::from_secs(timestamp_secs as u64)) {
                        // Aggiungi alla cache (con gestione limite)
                        self.add_to_cache(username.to_string(), system_time);
                        println!("✅ Successfully loaded timestamp for user '{}': {:?}", username, system_time);
                        return Ok(Some(system_time));
                    } else {
                        println!("❌ Failed to convert timestamp for user '{}'", username);
                    }
                }
                Ok(None) => {
                    println!("❌ No database entry found for user '{}'", username);
                    return Ok(None);
                }
                Err(e) => {
                    eprintln!("❌ Database query error for user '{}': {}", username, e);
                    return Ok(None);
                }
            }
        } else {
            println!("⚠️ Database pool not available for user '{}'", username);
        }
        
        println!("❌ No deletion timestamp found for user '{}'", username);
        Ok(None)
    }
    
    /// Marca una chat come eliminata (salvataggio asincrono)
    async fn mark_chat_deleted(&mut self, username: String) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let now = std::time::SystemTime::now();
        let timestamp_secs = now.duration_since(std::time::UNIX_EPOCH)?.as_secs() as i64;
        
        // Aggiorna cache immediatamente
        self.add_to_cache(username.clone(), now);
        
        // Salva nel database in modo asincrono
        if let Some(pool) = &self.db_pool {
            sqlx::query("INSERT OR REPLACE INTO deleted_chats (username, deleted_at) VALUES (?, ?)")
                .bind(&username)
                .bind(timestamp_secs)
                .execute(pool)
                .await?;
                
            println!("Chat deletion persisted for user: {}", username);
        }
        
        Ok(())
    }
    
    /// Gestione cache con limite di dimensione (LRU-like)
    fn add_to_cache(&mut self, username: String, timestamp: std::time::SystemTime) {
        if self.memory_cache.len() >= self.cache_size_limit {
            // Rimuovi alcune entry più vecchie (semplice cleanup)
            let mut entries: Vec<_> = self.memory_cache.iter().map(|(k, v)| (k.clone(), *v)).collect();
            entries.sort_by_key(|(_, time)| *time);
            
            // Rimuovi il 25% più vecchio
            let to_remove = self.cache_size_limit / 4;
            for (username_to_remove, _) in entries.iter().take(to_remove) {
                self.memory_cache.remove(username_to_remove);
            }
        }
        
        self.memory_cache.insert(username, timestamp);
    }
    
    /// Cleanup automatico di entry molto vecchie (>30 giorni)
    async fn cleanup_old_entries(&self) -> Result<usize, sqlx::Error> {
        if let Some(pool) = &self.db_pool {
            let thirty_days_ago = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs() as i64 - (30 * 24 * 60 * 60);
                
            let result = sqlx::query("DELETE FROM deleted_chats WHERE deleted_at < ?")
                .bind(thirty_days_ago)
                .execute(pool)
                .await?;
                
            println!("Cleaned up {} old deleted chat entries", result.rows_affected());
            return Ok(result.rows_affected() as usize);
        }
        Ok(0)
    }
}

// Struttura per mantenere la connessione persistente
#[derive(Debug)]
pub struct PersistentConnection {
    writer: BufWriter<tokio::net::tcp::OwnedWriteHalf>,
    reader: BufReader<tokio::net::tcp::OwnedReadHalf>,
}

#[derive(Debug, Clone, PartialEq, Eq, Copy)]
pub enum ConnectionMode {
    Localhost,
    Remote,
    Manual,
}

impl std::fmt::Display for ConnectionMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConnectionMode::Localhost => write!(f, "Localhost"),
            ConnectionMode::Remote => write!(f, "Remote Host"),
            ConnectionMode::Manual => write!(f, "Manual"),
        }
    }
}

#[derive(Debug, Clone)]
pub enum Message {
    // Connessione
    ConnectionModeSelected(ConnectionMode),
    ManualHostChanged(String),
    ManualPortChanged(String),
    ConnectPressed,
    Connected(Result<(), String>),
    ServerMessage(String),
    StartMessageListener,
    
    // Registrazione
    RegisterPressed,
    RegistrationSuccess(Arc<Mutex<PersistentConnection>>),
    
    // Disconnessione
    DisconnectPressed,
    
    // Input fields
    UsernameChanged(String),
    GroupNameChanged(String),
    InviteUsernameChanged(String),
    InviteGroupChanged(String),
    InviteIdChanged(String),
    LeaveGroupChanged(String),
    
    // Azioni principali
    ListUsersPressed,
    CreateGroupPressed,
    GroupCreated(String), // Nuovo: gruppo creato con successo, parametro è il nome del gruppo
    ListGroupsPressed,
    
    // Gestione inviti
    InviteUserPressed,
    ListInvitesPressed,
    AcceptInvitePressed,
    RejectInvitePressed,
    LeaveGroupPressed,
    
    // Espansione sezioni
    ToggleGroupsSection,
    ToggleInvitesSection,
    
    // Gestione sottosezioni Groups
    ShowGroupsList,
    ShowGroupsCreate,
    HideGroupsSubsections,
    
    // Azioni specifiche su elementi delle liste
    LeaveSpecificGroup(String), // group_id
    AcceptSpecificInvite(String),
    InviteToSpecificGroup(String), // group_id - usa l'ID del gruppo
    ShowInviteFormForGroup(String), // group_id - usa l'ID del gruppo
    
    // Gestione lista utenti per inviti 
    ShowUsersListForGroup(String), // group_id - usa l'ID del gruppo
    InviteUserToGroup(String, String), // (username, group_id)
    HideUsersListForGroup, // Nascondi lista utenti
    
    // Gestione form di invito per gruppo specifico
    InviteToGroupUsernameChanged(String),
    SendInviteToGroup, // Non ha bisogno del parametro nel match, usa invite_form_group invece
    CancelInviteToGroup,
    RejectSpecificInvite(String),
    
    // Aggiornamento delle liste
    GroupsListUpdated(Vec<(String, String)>), // (group_id, group_name)
    InvitesListUpdated(Vec<(String, String)>),
    UsersListUpdated(Vec<String>), // Lista utenti per inviti
    
    // Chat private
    StartPrivateChat(String), // username
    ClosePrivateChat,
    PrivateChatInputChanged(String),
    SendPrivateMessage,
    RefreshPrivateMessages(String), // username - ricarica messaggi dal server
    UpdatePrivateMessagesFromServer(String, String), // (username, server_response)
    NotificationReceived(String), // notifica dal server
    StartBackgroundListener, // avvia il listener per le notifiche
    StartPeriodicRefresh, // avvia refresh periodico per le chat
    TogglePrivateChatMenu, // Toggle del menu delle azioni chat privata
    
    // Chat di gruppo
    StartGroupChat(String, String), // (group_id, group_name)
    CloseGroupChat,
    GroupChatInputChanged(String),
    SendGroupMessage,
    
    // Gestione eliminazione messaggi
    DeleteGroupMessages(String), // group_id
    DeletePrivateMessages(String), // username
    ConfirmDeleteGroupMessages(String), // group_id
    CancelDelete,
    
    // Sistema di alert
    ShowAlert(String, AlertType),
    HideAlert,
    
    // Inizializzazione database scalabile
    InitializeDeletedChatsDatabase,
    DatabaseInitialized(DatabaseInitializationResult),
    ChatDeletionChecked(String, String, Option<std::time::SystemTime>), // (username, server_response, deletion_timestamp)
    
    // Dummy message per update interno
    None,
}

#[derive(Debug, Clone)]
pub enum AlertType {
    Success,
    Error,
    Warning,
    Info,
}

#[derive(Debug, Clone)]
pub struct Alert {
    message: String,
    alert_type: AlertType,
}

#[derive(Debug, Clone)]
pub enum AppState {
    Registration,  // Schermata iniziale per connessione e registrazione
    MainActions,   // Schermata principale con tutte le azioni
    Chat,          // Vista chat generale
    PrivateChat(String), // Chat privata con un utente specifico
    GroupChat(String, String), // Chat di gruppo (group_id, group_name)
}

#[derive(Debug, Clone)]
pub enum ConnectionState {
    Disconnected,
    Connecting,
    Connected,
    Registered,   // Nuovo stato: connesso E registrato
    Error(String),
}

pub struct ChatApp {
    // Stato dell'applicazione
    app_state: AppState,
    
    // Stato della connessione
    connection_state: ConnectionState,
    connection_mode: ConnectionMode,
    manual_host: String,
    manual_port: String,
    
    // Connessione persistente (solo quando registrati)
    persistent_connection: Option<Arc<Mutex<PersistentConnection>>>,
    
    // OTTIMIZZAZIONE: Flag per evitare richieste multiple simultanee
    is_command_pending: bool,
    
    // OTTIMIZZAZIONE: Cache per ridurre richieste al server
    users_cache_timestamp: Option<std::time::Instant>,
    groups_cache_timestamp: Option<std::time::Instant>,
    invites_cache_timestamp: Option<std::time::Instant>,
    private_chat_refresh_timestamp: Option<std::time::Instant>, // NUOVO: Per evitare refresh troppo frequenti
    
    // Input fields
    username: String,
    current_user_uuid: Option<String>, // UUID dell'utente corrente dal server
    group_name: String,
    invite_username: String,  // Username da invitare
    invite_group: String,     // Gruppo per l'invito
    invite_id: String,        // ID dell'invito da accettare/rifiutare
    leave_group_name: String, // Nome del gruppo da abbandonare
    
    // UI state - Sistema di alert al posto dei messaggi
    current_alert: Option<Alert>,
    
    // Stati delle sezioni espanse
    groups_expanded: bool,
    invites_expanded: bool,
    users_expanded: bool, // AGGIUNTO: Per rendere Users ricompattabile
    
    // Stati delle sottosezioni
    groups_list_view: bool,
    groups_create_view: bool,
    
    // Dati delle liste
    my_groups: Vec<(String, String)>, // (group_id, group_name)
    my_invites: Vec<(String, String)>, // (invite_id, group_name)
    available_users: Vec<String>, // Lista utenti disponibili per inviti
    
    // Gestione form di invito per gruppo specifico
    invite_form_group: Option<String>, // Group ID per cui è aperto il form di invito
    invite_to_group_username: String,  // Username da invitare nel form specifico
    users_list_for_group: Option<String>, // Group ID per cui è aperta la lista utenti
    
    // Chat privata
    private_chat_target: Option<String>, // Username del destinatario della chat privata
    private_chat_input: String,          // Testo del messaggio privato
    private_messages: std::collections::HashMap<String, Vec<(String, bool)>>, // Cronologia messaggi per utente (messaggio, is_sent_by_me)
    private_chat_menu_open: bool,        // Stato del menu delle azioni chat privata
    deleted_chats_manager: Arc<Mutex<ScalableDeletedChatsManager>>, // Manager scalabile per chat eliminate
    pending_sent_messages: std::collections::HashMap<String, Vec<String>>, // Messaggi inviati in attesa di conferma dal server
    
    // Chat di gruppo
    group_chat_target: Option<(String, String)>, // (group_id, group_name) della chat di gruppo attiva
    group_chat_input: String,                     // Testo del messaggio di gruppo
    group_messages: std::collections::HashMap<String, Vec<String>>, // Cronologia messaggi per gruppo (group_id -> messaggi)
    
    // Gestione eliminazione messaggi
    delete_confirmation: Option<DeleteConfirmation>,
    
    // Configurazione
    config: Option<ClientConfig>,
}

#[derive(Debug, Clone)]
pub struct DeleteConfirmation {
    pub delete_type: DeleteType,
    pub target: String, // group_id o username
}

#[derive(Debug, Clone)]
pub enum DeleteType {
    GroupMessages,
    PrivateMessages,
}

impl Application for ChatApp {
    type Message = Message;
    type Theme = Theme;
    type Executor = iced::executor::Default;
    type Flags = ();

    fn new(_flags: ()) -> (Self, Command<Message>) {
        env_logger::init();
        
        let config = match ClientConfig::load() {
            Ok(config) => Some(config),
            Err(e) => {
                error!("Failed to load config: {}", e);
                None
            }
        };
        
        // Carica i timestamp di eliminazione delle chat salvati (DEPRECATO - ora usiamo manager scalabile)
        let deleted_chats_manager = ScalableDeletedChatsManager::new();
        
        (
            Self {
                app_state: AppState::Registration,
                connection_state: ConnectionState::Disconnected,
                connection_mode: ConnectionMode::Localhost,
                manual_host: String::new(),
                manual_port: String::new(),
                persistent_connection: None,
                is_command_pending: false, // OTTIMIZZAZIONE: Inizializza il flag
                users_cache_timestamp: None, // OTTIMIZZAZIONE: Inizializza cache timestamp
                groups_cache_timestamp: None,
                invites_cache_timestamp: None,
                private_chat_refresh_timestamp: None, // NUOVO: Inizializza timestamp refresh chat
                username: String::new(),
                current_user_uuid: None,
                group_name: String::new(),
                invite_username: String::new(),
                invite_group: String::new(),
                invite_id: String::new(),
                leave_group_name: String::new(),
                current_alert: None,
                groups_expanded: false,
                invites_expanded: false,
                users_expanded: false, // AGGIUNTO: Inizializza Users come chiuso
                groups_list_view: false,
                groups_create_view: false,
                my_groups: Vec::new(),
                my_invites: Vec::new(),
                available_users: Vec::new(),
                invite_form_group: None,
                invite_to_group_username: String::new(),
                users_list_for_group: None,
                private_chat_target: None,
                private_chat_input: String::new(),
                private_messages: std::collections::HashMap::new(),
                private_chat_menu_open: false,
                deleted_chats_manager: Arc::new(Mutex::new(deleted_chats_manager)),
                pending_sent_messages: std::collections::HashMap::new(),
                group_chat_target: None,
                group_chat_input: String::new(),
                group_messages: std::collections::HashMap::new(),
                delete_confirmation: None,
                config,
            },
            {
                println!("📤 Sending InitializeDeletedChatsDatabase command from new()...");
                Command::perform(async {}, |_| Message::InitializeDeletedChatsDatabase)
            },
        )
    }

    fn title(&self) -> String {
        String::from("Ruggine Chat - GUI")
    }

    fn update(&mut self, message: Message) -> Command<Message> {
        match message {
            Message::ConnectionModeSelected(mode) => {
                self.connection_mode = mode;
                Command::none()
            }
            
            Message::ManualHostChanged(host) => {
                self.manual_host = host;
                Command::none()
            }
            
            Message::ManualPortChanged(port) => {
                self.manual_port = port;
                Command::none()
            }
            
            Message::ConnectPressed => {
                // Non più utilizzato - la connessione avviene con la registrazione
                Command::none()
            }
            
            Message::Connected(result) => {
                match result {
                    Ok(()) => {
                        self.connection_state = ConnectionState::Connected;
                        return Command::perform(async {}, |_| Message::ShowAlert(
                            "Connected to server!".to_string(),
                            AlertType::Success
                        ));
                    }
                    Err(error) => {
                        self.connection_state = ConnectionState::Error(error.clone());
                        return Command::perform(async {}, move |_| Message::ShowAlert(
                            format!("Connection failed: {}", error),
                            AlertType::Error
                        ));
                    }
                }
            }
            
            Message::StartMessageListener => {
                // Avvia il listener per i messaggi del server
                Command::none()
            }
            
            Message::InitializeDeletedChatsDatabase => {
                println!("🚀 Starting database initialization process...");
                Command::perform(
                    async move {
                        println!("🔧 Creating new ScalableDeletedChatsManager...");
                        let mut manager = ScalableDeletedChatsManager::new();
                        println!("🔧 Calling initialize_database()...");
                        match manager.initialize_database().await {
                            Ok(_) => {
                                println!("✅ Database initialization successful!");
                                DatabaseInitializationResult::Success(manager)
                            },
                            Err(e) => {
                                println!("❌ Database initialization failed: {}", e);
                                DatabaseInitializationResult::Error(e.to_string())
                            },
                        }
                    },
                    Message::DatabaseInitialized,
                )
            }

            Message::DatabaseInitialized(result) => {
                match result {
                    DatabaseInitializationResult::Success(manager) => {
                        self.deleted_chats_manager = Arc::new(Mutex::new(manager));
                        println!("✅ Deleted chats database successfully initialized and ready!");
                    }
                    DatabaseInitializationResult::Error(error) => {
                        eprintln!("❌ Failed to initialize deleted chats database: {}", error);
                        // Crea un manager di fallback vuoto
                        self.deleted_chats_manager = Arc::new(Mutex::new(ScalableDeletedChatsManager::new()));
                    }
                }
                Command::none()
            }
            
            Message::ServerMessage(msg) => {
                if !msg.trim().is_empty() {
                    Command::perform(async {}, move |_| Message::ShowAlert(
                        format!("SERVER: {}", msg.trim()),
                        AlertType::Info
                    ))
                } else {
                    Command::none()
                }
            }
            
            Message::UsernameChanged(username) => {
                self.username = username;
                Command::none()
            }
            
            Message::GroupNameChanged(name) => {
                self.group_name = name;
                Command::none()
            }
            
            Message::RegisterPressed => {
                let username = self.username.clone();
                if !username.is_empty() {
                    if let Some((host, port)) = self.get_connection_info() {
                        self.connection_state = ConnectionState::Connecting;
                        
                        // Prima mostra l'alert di connecting
                        let username_for_alert = username.clone();
                        let alert_command = Command::perform(async {}, move |_| Message::ShowAlert(
                            format!("Connecting and registering as '{}'...", username_for_alert),
                            AlertType::Info
                        ));
                        
                        // Poi esegui la connessione e registrazione
                        let connect_command = Command::perform(
                            Self::connect_and_register_persistent(host, port, username),
                            |result| match result {
                                Ok((response, connection)) => {
                                    if response.contains("OK:") {
                                        Message::RegistrationSuccess(connection)
                                    } else {
                                        Message::ServerMessage(format!("Registration failed: {}", response))
                                    }
                                }
                                Err(e) => Message::ServerMessage(format!("Connection error: {}", e))
                            }
                        );
                        
                        Command::batch(vec![alert_command, connect_command])
                    } else {
                        Command::perform(async {}, |_| Message::ShowAlert(
                            "Connection info not available".to_string(),
                            AlertType::Error
                        ))
                    }
                } else {
                    Command::perform(async {}, |_| Message::ShowAlert(
                        "WARNING: Username cannot be empty".to_string(),
                        AlertType::Warning
                    ))
                }
            }
            
            Message::RegistrationSuccess(connection) => {
                self.connection_state = ConnectionState::Registered;
                self.app_state = AppState::MainActions;
                self.persistent_connection = Some(connection.clone());
                
                let alert_command = Command::perform(async {}, |_| Message::ShowAlert(
                    "Registration successful! Welcome to Ruggine Chat.".to_string(),
                    AlertType::Success
                ));
                
                let listener_command = Command::perform(async {}, |_| Message::StartBackgroundListener);
                let refresh_command = Command::perform(async {}, |_| Message::StartPeriodicRefresh);
                
                Command::batch(vec![alert_command, listener_command, refresh_command])
            }
            
            Message::DisconnectPressed => {
                // Chiudi la connessione e torna alla schermata di registrazione
                self.persistent_connection = None;
                self.connection_state = ConnectionState::Disconnected;
                self.app_state = AppState::Registration;
                
                // Pulisci tutti i campi del form
                self.username.clear();
                self.current_user_uuid = None;
                self.group_name.clear();
                self.invite_username.clear();
                self.invite_group.clear();
                self.invite_id.clear();
                self.leave_group_name.clear();
                self.invite_to_group_username.clear();
                
                // Reset degli stati UI
                self.groups_expanded = false;
                self.invites_expanded = false;
                self.groups_list_view = false;
                self.groups_create_view = false;
                self.invite_form_group = None;
                self.users_list_for_group = None;
                
                // CORREZIONE: Reset cache timestamps
                self.users_cache_timestamp = None;
                self.groups_cache_timestamp = None;
                self.invites_cache_timestamp = None;
                self.private_chat_refresh_timestamp = None; // NUOVO: Reset anche timestamp refresh chat
                
                // Pulisci le liste
                self.my_groups.clear();
                self.my_invites.clear();
                self.available_users.clear();
                
                // Pulisci chat privata e di gruppo
                self.private_chat_target = None;
                self.private_chat_input.clear();
                self.private_messages.clear();
                self.private_chat_menu_open = false;
                // MANTIENI: deleted_chats_manager NON viene cancellato - è persistente tra sessioni via database!
                self.pending_sent_messages.clear(); // Reset messaggi in attesa
                self.group_chat_target = None;
                self.group_chat_input.clear();
                self.group_messages.clear();
                self.delete_confirmation = None;
                
                Command::perform(async {}, |_| Message::ShowAlert(
                    "Disconnected from server.".to_string(),
                    AlertType::Info
                ))
            }
            
            Message::ListUsersPressed => {
                // Toggle users section
                if self.users_expanded {
                    // Se è già espanso, chiudilo
                    self.users_expanded = false;
                    self.available_users.clear();
                    Command::none()
                } else {
                    // Se è chiuso, aprilo e carica gli utenti
                    self.users_expanded = true;
                    if matches!(self.connection_state, ConnectionState::Registered) {
                        if let Some(connection) = &self.persistent_connection {
                            let conn = connection.clone();
                            // Mostra alert e invia comando
                            let alert_command = Command::perform(async {}, |_| Message::ShowAlert(
                                "Requesting user list...".to_string(),
                                AlertType::Info
                            ));
                            let send_command = Command::perform(
                                Self::send_command_persistent(conn, "/all_users".to_string()),
                                |result| match result {
                                    Ok(response) => {
                                        // Parsa gli utenti e aggiorna la lista
                                        let users = Self::parse_users_response(&response);
                                        Message::UsersListUpdated(users)
                                    },
                                    Err(e) => Message::ServerMessage(format!("Error: {}", e))
                                }
                            );
                            Command::batch(vec![alert_command, send_command])
                        } else {
                            Command::perform(async {}, |_| Message::ShowAlert(
                                "No persistent connection available".to_string(),
                                AlertType::Error
                            ))
                        }
                    } else {
                        Command::perform(async {}, |_| Message::ShowAlert(
                            "Please register first".to_string(),
                            AlertType::Warning
                        ))
                    }
                }
            }
            
            Message::CreateGroupPressed => {
                if matches!(self.connection_state, ConnectionState::Registered) {
                    let group_name = self.group_name.clone();
                    if !group_name.is_empty() {
                        if let Some(connection) = &self.persistent_connection {
                            let conn = connection.clone();
                            let command = format!("/create_group {}", group_name);
                            let group_name_for_closure = group_name.clone();
                            // Mostra alert e invia comando
                            let alert_command = Command::perform(async {}, move |_| Message::ShowAlert(
                                format!("Creating group '{}'...", group_name),
                                AlertType::Info
                            ));
                            let send_command = Command::perform(
                                Self::send_command_persistent(conn, command),
                                move |result| match result {
                                    Ok(response) => {
                                        if response.contains("successfully") || response.starts_with("OK:") {
                                            Message::GroupCreated(group_name_for_closure)
                                        } else {
                                            Message::ServerMessage(format!("[Group] {}", response))
                                        }
                                    },
                                    Err(e) => Message::ServerMessage(format!("Error: {}", e))
                                }
                            );
                            Command::batch(vec![alert_command, send_command])
                        } else {
                            return Command::perform(async {}, |_| Message::ShowAlert(
                                "No persistent connection available".to_string(),
                                AlertType::Error
                            ));
                        }
                    } else {
                        return Command::perform(async {}, |_| Message::ShowAlert(
                            "WARNING: Group name cannot be empty".to_string(),
                            AlertType::Warning
                        ));
                    }
                } else {
                    return Command::perform(async {}, |_| Message::ShowAlert(
                        "Please register first".to_string(),
                        AlertType::Warning
                    ));
                }
            }
            
            Message::GroupCreated(group_name) => {
                // Pulisce il campo group_name e mostra alert di successo
                self.group_name.clear();
                
                // Chiudi il form di creazione gruppo, SENZA aprire automaticamente la lista
                self.groups_create_view = false;
                // CORREZIONE: NON aprire automaticamente la lista gruppi
                
                // INVALIDAZIONE CACHE: Forza refresh dei gruppi per la prossima apertura
                self.groups_cache_timestamp = None;
                
                // CORREZIONE: Solo se la lista è già aperta, aggiornala
                if self.groups_list_view {
                    if let Some(connection) = &self.persistent_connection {
                        let conn = connection.clone();
                        let refresh_command = Command::perform(
                            Self::send_command_persistent(conn, "/my_groups".to_string()),
                            |result| match result {
                                Ok(response) => {
                                    let groups = Self::parse_groups_response(&response);
                                    Message::GroupsListUpdated(groups)
                                }
                                Err(e) => Message::ServerMessage(format!("Error refreshing groups: {}", e))
                            }
                        );
                        
                        let alert_command = Command::perform(async {}, move |_| Message::ShowAlert(
                            format!("Group '{}' created successfully!", group_name),
                            AlertType::Success
                        ));
                        
                        return Command::batch(vec![alert_command, refresh_command]);
                    }
                }
                
                Command::perform(async {}, move |_| Message::ShowAlert(
                    format!("Group '{}' created successfully!", group_name),
                    AlertType::Success
                ))
            }
            
            
            Message::ListGroupsPressed => {
                if matches!(self.connection_state, ConnectionState::Registered) {
                    if let Some(connection) = &self.persistent_connection {
                        let conn = connection.clone();
                        // Mostra alert e invia comando
                        let alert_command = Command::perform(async {}, |_| Message::ShowAlert(
                            "Requesting my groups...".to_string(),
                            AlertType::Info
                        ));
                        let send_command = Command::perform(
                            Self::send_command_persistent(conn, "/my_groups".to_string()),
                            |result| match result {
                                Ok(response) => {
                                    // Parse la risposta per estrarre i nomi dei gruppi
                                    let groups = Self::parse_groups_response(&response);
                                    Message::GroupsListUpdated(groups)
                                }
                                Err(e) => Message::ServerMessage(format!("Error: {}", e))
                            }
                        );
                        Command::batch(vec![alert_command, send_command])
                    } else {
                        return Command::perform(async {}, |_| Message::ShowAlert(
                            "No persistent connection available".to_string(),
                            AlertType::Error
                        ));
                    }
                } else {
                    return Command::perform(async {}, |_| Message::ShowAlert(
                        "Please register first".to_string(),
                        AlertType::Warning
                    ));
                }
            }
            
            Message::InviteUsernameChanged(username) => {
                self.invite_username = username;
                Command::none()
            }
            
            Message::InviteGroupChanged(group) => {
                self.invite_group = group;
                Command::none()
            }
            
            Message::InviteIdChanged(id) => {
                self.invite_id = id;
                Command::none()
            }
            
            Message::LeaveGroupChanged(group) => {
                self.leave_group_name = group;
                Command::none()
            }
            
            Message::InviteUserPressed => {
                if matches!(self.connection_state, ConnectionState::Registered) {
                    let username = self.invite_username.clone();
                    let group_name = self.invite_group.clone();
                    
                    if !username.is_empty() && !group_name.is_empty() {
                        if let Some(connection) = &self.persistent_connection {
                            let conn = connection.clone();
                            let command = format!("/invite {} {}", username, group_name);
                            // Mostra alert e invia comando
                            let alert_command = Command::perform(async {}, move |_| Message::ShowAlert(
                                format!("Inviting '{}' to group '{}'...", username, group_name),
                                AlertType::Info
                            ));
                            let send_command = Command::perform(
                                Self::send_command_persistent(conn, command),
                                |result| match result {
                                    Ok(response) => Message::ServerMessage(format!("[Invite] {}", response)),
                                    Err(e) => Message::ServerMessage(format!("Error: {}", e))
                                }
                            );
                            Command::batch(vec![alert_command, send_command])
                        } else {
                            return Command::perform(async {}, |_| Message::ShowAlert(
                                "No persistent connection available".to_string(),
                                AlertType::Error
                            ));
                        }
                    } else {
                        return Command::perform(async {}, |_| Message::ShowAlert(
                            "WARNING: Username and group name cannot be empty".to_string(),
                            AlertType::Warning
                        ));
                    }
                } else {
                    return Command::perform(async {}, |_| Message::ShowAlert(
                        "Please register first".to_string(),
                        AlertType::Warning
                    ));
                }
            }
            
            Message::ListInvitesPressed => {
                if matches!(self.connection_state, ConnectionState::Registered) {
                    if let Some(connection) = &self.persistent_connection {
                        let conn = connection.clone();
                        // Mostra alert e invia comando
                        let alert_command = Command::perform(async {}, |_| Message::ShowAlert(
                            "Requesting pending invites...".to_string(),
                            AlertType::Info
                        ));
                        let send_command = Command::perform(
                            Self::send_command_persistent(conn, "/my_invites".to_string()),
                            |result| match result {
                                Ok(response) => {
                                    // Parse la risposta per estrarre gli inviti
                                    let invites = Self::parse_invites_response(&response);
                                    Message::InvitesListUpdated(invites)
                                }
                                Err(e) => Message::ServerMessage(format!("Error: {}", e))
                            }
                        );
                        Command::batch(vec![alert_command, send_command])
                    } else {
                        return Command::perform(async {}, |_| Message::ShowAlert(
                            "No persistent connection available".to_string(),
                            AlertType::Error
                        ));
                    }
                } else {
                    return Command::perform(async {}, |_| Message::ShowAlert(
                        "Please register first".to_string(),
                        AlertType::Warning
                    ));
                }
            }
            
            Message::AcceptInvitePressed => {
                if matches!(self.connection_state, ConnectionState::Registered) {
                    let invite_id = self.invite_id.clone();
                    
                    if !invite_id.is_empty() {
                        if let Some(connection) = &self.persistent_connection {
                            // Converted to alert system
                            let conn = connection.clone();
                            let command = format!("/accept_invite {}", invite_id);
                            Command::perform(
                                Self::send_command_persistent(conn, command),
                                |result| match result {
                                    Ok(response) => Message::ServerMessage(format!("[Accept] {}", response)),
                                    Err(e) => Message::ServerMessage(format!("Error: {}", e))
                                }
                            )
                        } else {
                            // Converted to alert system
                            Command::none()
                        }
                    } else {
                        // Converted to alert system
                        Command::none()
                    }
                } else {
                    // Converted to alert system
                    Command::none()
                }
            }
            
            Message::RejectInvitePressed => {
                if matches!(self.connection_state, ConnectionState::Registered) {
                    let invite_id = self.invite_id.clone();
                    
                    if !invite_id.is_empty() {
                        if let Some(connection) = &self.persistent_connection {
                            // Converted to alert system
                            let conn = connection.clone();
                            let command = format!("/reject_invite {}", invite_id);
                            Command::perform(
                                Self::send_command_persistent(conn, command),
                                |result| match result {
                                    Ok(response) => Message::ServerMessage(format!("[Reject] {}", response)),
                                    Err(e) => Message::ServerMessage(format!("Error: {}", e))
                                }
                            )
                        } else {
                            // Converted to alert system
                            Command::none()
                        }
                    } else {
                        // Converted to alert system
                        Command::none()
                    }
                } else {
                    // Converted to alert system
                    Command::none()
                }
            }
            
            Message::LeaveGroupPressed => {
                if matches!(self.connection_state, ConnectionState::Registered) {
                    let group_name = self.leave_group_name.clone();
                    
                    if !group_name.is_empty() {
                        if let Some(connection) = &self.persistent_connection {
                            // Converted to alert system
                            let conn = connection.clone();
                            let command = format!("/leave_group {}", group_name);
                            Command::perform(
                                Self::send_command_persistent(conn, command),
                                |result| match result {
                                    Ok(response) => Message::ServerMessage(format!("[Leave] {}", response)),
                                    Err(e) => Message::ServerMessage(format!("Error: {}", e))
                                }
                            )
                        } else {
                            // Converted to alert system
                            Command::none()
                        }
                    } else {
                        // Converted to alert system
                        Command::none()
                    }
                } else {
                    // Converted to alert system
                    Command::none()
                }
            }
            
            Message::ToggleGroupsSection => {
                self.groups_expanded = !self.groups_expanded;
                // Reset delle sottosezioni quando si chiude/apre la sezione principale
                if !self.groups_expanded {
                    self.groups_list_view = false;
                    self.groups_create_view = false;
                }
                Command::none()
            }
            
            Message::ToggleInvitesSection => {
                self.invites_expanded = !self.invites_expanded;
                
                // Quando si apre la sezione inviti, carica automaticamente la lista solo se necessario
                if self.invites_expanded {
                    // OTTIMIZZAZIONE: Usa cache se recente (meno di 30 secondi per inviti)
                    let should_fetch = if let Some(timestamp) = self.invites_cache_timestamp {
                        timestamp.elapsed() > std::time::Duration::from_secs(30)
                    } else {
                        true
                    };
                    
                    if should_fetch && matches!(self.connection_state, ConnectionState::Registered) {
                        if let Some(connection) = &self.persistent_connection {
                            let conn = connection.clone();
                            return Command::perform(
                                Self::send_command_persistent(conn, "/my_invites".to_string()),
                                |result| match result {
                                    Ok(response) => {
                                        let invites = Self::parse_invites_response(&response);
                                        Message::InvitesListUpdated(invites)
                                    }
                                    Err(e) => Message::ServerMessage(format!("Error: {}", e))
                                }
                            );
                        }
                    }
                }
                Command::none()
            }
            
            Message::ShowGroupsList => {
                // Toggle groups list view
                if self.groups_list_view {
                    // Se è già in vista lista, chiudila (mantieni la cache)
                    self.groups_list_view = false;
                    Command::none()
                } else {
                    // Se non è in vista lista, aprila
                    self.groups_list_view = true;
                    self.groups_create_view = false;
                    
                    // OTTIMIZZAZIONE: Fetch solo se cache vuota o molto vecchia (5 minuti)
                    let should_fetch = if let Some(timestamp) = self.groups_cache_timestamp {
                        timestamp.elapsed() > std::time::Duration::from_secs(300) // 5 minuti
                    } else {
                        true // CORREZIONE: Se non c'è timestamp, fetch sempre
                    };
                    
                    if should_fetch {
                        // Carica la lista dei gruppi
                        if matches!(self.connection_state, ConnectionState::Registered) {
                            if let Some(connection) = &self.persistent_connection {
                                let conn = connection.clone();
                                return Command::perform(
                                    Self::send_command_persistent(conn, "/my_groups".to_string()),
                                    |result| match result {
                                        Ok(response) => {
                                            let groups = Self::parse_groups_response(&response);
                                            Message::GroupsListUpdated(groups)
                                        }
                                        Err(e) => Message::ServerMessage(format!("Error: {}", e))
                                    }
                                );
                            } else {
                                return Command::perform(async {}, |_| Message::ShowAlert(
                                    "No persistent connection available".to_string(),
                                    AlertType::Error
                                ));
                            }
                        } else {
                            return Command::perform(async {}, |_| Message::ShowAlert(
                                "Please register first".to_string(),
                                AlertType::Warning
                            ));
                        }
                    } else {
                        // Usa cache esistente - nessun fetch necessario
                        Command::none()
                    }
                }
            }
            
            Message::ShowGroupsCreate => {
                self.groups_create_view = true;
                self.groups_list_view = false;
                Command::none()
            }
            
            Message::HideGroupsSubsections => {
                self.groups_list_view = false;
                self.groups_create_view = false;
                Command::none()
            }
            
            Message::LeaveSpecificGroup(group_id) => {
                if matches!(self.connection_state, ConnectionState::Registered) {
                    if let Some(connection) = &self.persistent_connection {
                        // Invalida la cache dei gruppi
                        self.groups_cache_timestamp = None;
                        
                        let conn = connection.clone();
                        let command = format!("/leave_group_by_id {}", group_id);
                        
                        Command::perform(
                            Self::send_command_persistent(conn, command),
                            |result| match result {
                                Ok(response) => {
                                    Message::ServerMessage(format!("[Leave] {} - Groups cache refreshed", response))
                                }
                                Err(e) => Message::ServerMessage(format!("Error: {}", e))
                            }
                        )
                    } else {
                        Command::none()
                    }
                } else {
                    Command::none()
                }
            }
            
            Message::AcceptSpecificInvite(invite_id) => {
                if matches!(self.connection_state, ConnectionState::Registered) {
                    if let Some(connection) = &self.persistent_connection {
                        // Chiudi la sezione inviti dopo aver accettato
                        self.invites_expanded = false;
                        
                        // Invalida la cache dei gruppi (nuovo gruppo aggiunto)
                        self.groups_cache_timestamp = None;
                        // Invalida la cache degli inviti (invito rimosso)
                        self.invites_cache_timestamp = None;
                        
                        let conn = connection.clone();
                        let command = format!("/accept_invite {}", invite_id);
                        let _show_groups = self.groups_list_view; // Cattura lo stato attuale
                        
                        let alert_command = Command::perform(async {}, |_| Message::ShowAlert(
                            "Accepting invite...".to_string(),
                            AlertType::Info
                        ));
                        
                        let send_command = Command::perform(
                            Self::send_command_persistent(conn, command),
                            |result| match result {
                                Ok(response) => {
                                    Message::ServerMessage(format!("[Accept] {} - Groups and invites cache refreshed", response))
                                }
                                Err(e) => Message::ServerMessage(format!("Error: {}", e))
                            }
                        );
                        
                        Command::batch(vec![alert_command, send_command])
                    } else {
                        // Converted to alert system
                        Command::none()
                    }
                } else {
                    // Converted to alert system
                    Command::none()
                }
            }
            
            Message::RejectSpecificInvite(invite_id) => {
                if matches!(self.connection_state, ConnectionState::Registered) {
                    if let Some(connection) = &self.persistent_connection {
                        // Chiudi la sezione inviti dopo aver rifiutato
                        self.invites_expanded = false;
                        
                        // Invalida la cache degli inviti
                        self.invites_cache_timestamp = None;
                        
                        let conn = connection.clone();
                        let command = format!("/reject_invite {}", invite_id);
                        let _invites_expanded = self.invites_expanded; // Cattura lo stato attuale
                        
                        let alert_command = Command::perform(async {}, |_| Message::ShowAlert(
                            "Rejecting invite...".to_string(),
                            AlertType::Info
                        ));
                        
                        let send_command = Command::perform(
                            Self::send_command_persistent(conn, command),
                            |result| match result {
                                Ok(response) => {
                                    Message::ServerMessage(format!("[Reject] {} - Invites cache refreshed", response))
                                }
                                Err(e) => Message::ServerMessage(format!("Error: {}", e))
                            }
                        );
                        
                        Command::batch(vec![alert_command, send_command])
                    } else {
                        // Converted to alert system
                        Command::none()
                    }
                } else {
                    // Converted to alert system
                    Command::none()
                }
            }
            
            Message::GroupsListUpdated(groups) => {
                self.my_groups = groups;
                self.groups_cache_timestamp = Some(std::time::Instant::now()); // OTTIMIZZAZIONE: Aggiorna cache
                Command::none()
            }
            
            Message::InvitesListUpdated(invites) => {
                self.my_invites = invites;
                self.invites_cache_timestamp = Some(std::time::Instant::now()); // OTTIMIZZAZIONE: Aggiorna cache
                Command::none()
            }
            
            Message::UsersListUpdated(users) => {
                self.available_users = users;
                self.is_command_pending = false; // OTTIMIZZAZIONE: Reset flag
                self.users_cache_timestamp = Some(std::time::Instant::now()); // OTTIMIZZAZIONE: Aggiorna timestamp cache
                Command::none()
            }
            
            // Chat private
            Message::StartPrivateChat(username) => {
                self.private_chat_target = Some(username.clone());
                self.app_state = AppState::PrivateChat(username.clone());
                self.private_chat_input.clear();
                self.private_chat_menu_open = false; // Reset del menu quando si apre una nuova chat
                
                // CORREZIONE: Se non abbiamo l'UUID, prova a ottenerlo inviando un messaggio speciale
                if self.current_user_uuid.is_none() {
                    if let Some(connection) = &self.persistent_connection {
                        let conn = connection.clone();
                        let discovery_message = format!("__UUID_DISCOVERY_{}", std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_millis());
                        let command = format!("/private {} {}", username, discovery_message);
                        
                        // Traccia questo messaggio speciale per il discovery
                        self.pending_sent_messages
                            .entry(username.clone())
                            .or_insert_with(Vec::new)
                            .push(discovery_message);
                        
                        println!("Sending UUID discovery message for user: {}", username);
                        
                        // Invia il messaggio di discovery e poi ricarica
                        let username_for_refresh = username.clone();
                        let discovery_command = Command::perform(
                            Self::send_command_persistent(conn, command),
                            move |result| match result {
                                Ok(_) => Message::RefreshPrivateMessages(username_for_refresh),
                                Err(_) => Message::RefreshPrivateMessages(username_for_refresh)
                            }
                        );
                        
                        return discovery_command;
                    }
                }
                
                // Ricarica automaticamente i messaggi dal server
                return Command::perform(async { username }, |username| Message::RefreshPrivateMessages(username));
            }
            
            Message::ClosePrivateChat => {
                self.private_chat_target = None;
                self.app_state = AppState::Chat;
                self.private_chat_input.clear();
                self.private_chat_menu_open = false; // Reset del menu quando si chiude la chat
                Command::none()
            }
            
            Message::TogglePrivateChatMenu => {
                self.private_chat_menu_open = !self.private_chat_menu_open;
                Command::none()
            }
            
            Message::PrivateChatInputChanged(input) => {
                self.private_chat_input = input;
                Command::none()
            }
            
            Message::SendPrivateMessage => {
                if let Some(target_user) = &self.private_chat_target {
                    if !self.private_chat_input.trim().is_empty() {
                        if let Some(connection) = &self.persistent_connection {
                            let conn = connection.clone();
                            let message_text = self.private_chat_input.trim().to_string();
                            let message = format!("/private {} {}", target_user, message_text);
                            
                            // NON rimuovere il timestamp di eliminazione!
                            // Se la chat è stata eliminata, mantieni il filtro ma aggiungi il messaggio ai pending
                            // così verrà mostrato quando arriva dal server
                            
                            // Traccia questo messaggio come "in attesa di conferma dal server"
                            self.pending_sent_messages
                                .entry(target_user.clone())
                                .or_insert_with(Vec::new)
                                .push(message_text.clone());
                            
                            self.private_chat_input.clear();
                            
                            let send_command = Command::perform(
                                Self::send_command_persistent(conn, message),
                                |result| match result {
                                    Ok(_) => Message::ShowAlert("Message sent".to_string(), AlertType::Success),
                                    Err(e) => Message::ShowAlert(format!("Error: {}", e), AlertType::Error)
                                }
                            );
                            
                            // Aggiungi un piccolo delay e poi refresh per vedere il messaggio dal server
                            let target_user_clone = target_user.clone();
                            let refresh_command = Command::perform(
                                async move {
                                    tokio::time::sleep(tokio::time::Duration::from_millis(300)).await;
                                    target_user_clone
                                },
                                |username| Message::RefreshPrivateMessages(username)
                            );
                            
                            return Command::batch(vec![send_command, refresh_command]);
                        }
                    }
                }
                Command::none()
            }
            
            Message::RefreshPrivateMessages(username) => {
                if let Some(connection) = &self.persistent_connection {
                    let conn = connection.clone();
                    let command = format!("/get_private_messages {}", username);
                    println!("Refreshing private messages for {}: {}", username, command);
                    
                    return Command::perform(
                        Self::send_command_persistent(conn, command),
                        |result| match result {
                            Ok(response) => {
                                println!("Get messages response: {}", response);
                                // Parse la risposta e aggiorna i messaggi
                                Message::UpdatePrivateMessagesFromServer(username, response)
                            }
                            Err(e) => {
                                println!("Get messages error: {}", e);
                                Message::ShowAlert(format!("Failed to refresh messages: {}", e), AlertType::Error)
                            }
                        }
                    );
                }
                Command::none()
            }
            
            Message::UpdatePrivateMessagesFromServer(username, server_response) => {
                println!("Updating private messages from server for {}: {}", username, server_response);
                
                // Controllo asincrono per ottenere il timestamp di eliminazione
                let manager = self.deleted_chats_manager.clone();
                let username_clone = username.clone();
                let response_clone = server_response.clone();
                
                Command::perform(
                    async move {
                        let mut manager_lock = manager.lock().await;
                        let deletion_timestamp = manager_lock.get_deletion_timestamp(&username_clone).await.unwrap_or(None);
                        println!("🔍 Database check for user '{}': deletion_timestamp = {:?}", username_clone, deletion_timestamp);
                        (username_clone, response_clone, deletion_timestamp)
                    },
                    |(username, response, deletion_timestamp)| Message::ChatDeletionChecked(username, response, deletion_timestamp)
                )
            }

            Message::ChatDeletionChecked(username, server_response, deletion_timestamp) => {
                if let Some(deletion_time) = deletion_timestamp {
                    println!("User {} has deleted chat at {:?}, filtering messages older than deletion", username, deletion_time);
                } else {
                    println!("User {} has not deleted chat, showing all messages", username);
                }
                
                // Parse la risposta del server e aggiorna i messaggi per questo utente
                if server_response.starts_with("OK:") {
                    // Ottieni la lista dei messaggi pending per questo utente
                    let mut pending_messages = self.pending_sent_messages.get(&username).cloned().unwrap_or_default();
                    
                    // Prova a estrarre l'UUID dell'utente corrente se non lo abbiamo ancora
                    if self.current_user_uuid.is_none() {
                        // CORREZIONE: Prova sempre a rilevare l'UUID, anche senza pending messages
                        // Cerca nell'intera risposta del server per trovare un pattern che ci aiuti
                        for line in server_response.lines() {
                            // Se abbiamo pending messages, cerca quelli prima
                            for pending_msg in &pending_messages {
                                if line.contains(pending_msg) {
                                    // Estrai l'UUID da questa riga
                                    if let Some(colon_pos) = line.rfind(": ") {
                                        let before_colon = &line[..colon_pos];
                                        if let Some(bracket_pos) = before_colon.rfind("] ") {
                                            let uuid_part = &before_colon[bracket_pos + 2..];
                                            if uuid_part.len() > 20 && uuid_part.contains('-') {
                                                println!("DISCOVERED current user UUID: '{}'", uuid_part);
                                                self.current_user_uuid = Some(uuid_part.to_string());
                                                break;
                                            }
                                        }
                                    }
                                }
                            }
                            if self.current_user_uuid.is_some() {
                                break;
                            }
                        }
                        
                        // NUOVO: Se non abbiamo pending messages, prova un approccio diverso
                        // Cerca messaggi che potrebbero essere nostri basandoci su timing o altri pattern
                        if self.current_user_uuid.is_none() && pending_messages.is_empty() {
                            // Nella prossima versione potremmo aggiungere logica più sofisticata qui
                            // Per ora, se non riusciamo a determinare l'UUID, i messaggi saranno mostrati come ricevuti
                            println!("Cannot determine current user UUID without pending messages");
                        }
                    }
                    
                    let new_messages = Self::parse_private_messages_response_with_filter(
                        &server_response, 
                        &self.username, 
                        self.current_user_uuid.as_deref(), 
                        &mut pending_messages,
                        deletion_timestamp.as_ref() // Usa il timestamp di eliminazione per filtrare
                    );
                    println!("Parsed {} messages from server", new_messages.len());
                    
                    // Aggiorna la lista dei pending messages (sono stati rimossi quelli matchati)
                    if pending_messages.is_empty() {
                        self.pending_sent_messages.remove(&username);
                    } else {
                        self.pending_sent_messages.insert(username.clone(), pending_messages);
                    }
                    
                    // CORREZIONE: Sostituisci COMPLETAMENTE i messaggi con quelli dal server
                    // Il server ha la verità assoluta sui messaggi, mantiene l'ordine corretto
                    self.private_messages.insert(username, new_messages);
                } else {
                    println!("Server response indicates error or no messages: {}", server_response);
                    // La risposta indica un errore o nessun messaggio
                    if server_response.contains("No messages found") {
                        // Se il server dice che non ci sono messaggi, pulisci tutto
                        self.private_messages.insert(username, Vec::new());
                    }
                }
                Command::none()
            }
            
            Message::NotificationReceived(notification) => {
                println!("Received notification: {}", notification);
                
                // Gestisce le notifiche dal server (es. "NOTIFICATION:PRIVATE_MESSAGE:username")
                if notification == "CONNECTION_CLOSED" {
                    // La connessione è stata chiusa, non fare nulla
                    return Command::none();
                }
                
                if notification == "CONTINUE_LISTENING" {
                    // OTTIMIZZAZIONE: Riavvia il listener senza triggering aggiuntivi
                    if let Some(connection) = &self.persistent_connection {
                        let conn_clone = connection.clone();
                        return Command::perform(Self::background_notification_listener(conn_clone), |notification| {
                            Message::NotificationReceived(notification)
                        });
                    }
                    return Command::none();
                }
                
                let mut commands = Vec::new();
                
                if notification.starts_with("NOTIFICATION:PRIVATE_MESSAGE:") {
                    let username = notification.strip_prefix("NOTIFICATION:PRIVATE_MESSAGE:").unwrap_or("").to_string();
                    println!("Private message notification from: {}", username);
                    
                    if !username.is_empty() {
                        // Crea copie per evitare problemi di ownership
                        let username_for_first_refresh = username.clone();
                        let username_for_comparison = username.clone();
                        
                        // Aggiorna sempre i messaggi quando arriva una notifica, 
                        // indipendentemente dalla chat attiva
                        commands.push(Command::perform(async move { username_for_first_refresh }, |username| Message::RefreshPrivateMessages(username)));
                        
                        // Se stiamo visualizzando la chat con questo utente, forza un refresh immediato
                        if let Some(target_username) = &self.private_chat_target {
                            if target_username == &username_for_comparison {
                                commands.push(Command::perform(async move { username_for_comparison }, |username| Message::RefreshPrivateMessages(username)));
                            }
                        }
                    }
                }
                
                // Riavvia il background listener per continuare ad ascoltare
                if let Some(connection) = &self.persistent_connection {
                    let conn_clone = connection.clone();
                    commands.push(Command::perform(Self::background_notification_listener(conn_clone), |notification| {
                        Message::NotificationReceived(notification)
                    }));
                }
                
                Command::batch(commands)
            }
            
            Message::StartBackgroundListener => {
                if let Some(connection) = &self.persistent_connection {
                    let conn_clone = connection.clone();
                    return Command::perform(Self::background_notification_listener(conn_clone), |notification| {
                        Message::NotificationReceived(notification)
                    });
                }
                Command::none()
            }
            
            Message::StartPeriodicRefresh => {
                // OTTIMIZZATO: Refresh molto meno frequente (da 10s a 30s) per ridurre drasticamente il carico
                let mut refresh_commands = Vec::new();
                
                // Solo refresh per la chat attiva corrente CON throttling
                if let Some(target_username) = &self.private_chat_target {
                    // OTTIMIZZAZIONE: Controlla se l'ultimo refresh è troppo recente (< 10 secondi)
                    let should_refresh = if let Some(last_refresh) = self.private_chat_refresh_timestamp {
                        last_refresh.elapsed() > std::time::Duration::from_secs(10)
                    } else {
                        true // Primo refresh sempre consentito
                    };
                    
                    if should_refresh {
                        let username = target_username.clone();
                        refresh_commands.push(Command::perform(async move { username }, |username| Message::RefreshPrivateMessages(username)));
                        // Aggiorna timestamp ultimo refresh
                        self.private_chat_refresh_timestamp = Some(std::time::Instant::now());
                    }
                }
                
                // OTTIMIZZATO: Non aggiornare più tutte le chat inattive per ridurre il carico
                // Solo la chat attiva viene aggiornata periodicamente
                
                // OTTIMIZZATO: Continua il ciclo di refresh con intervallo molto più lungo (30s invece di 10s)
                let continue_command = Command::perform(async {
                    tokio::time::sleep(tokio::time::Duration::from_secs(30)).await;
                }, |_| Message::StartPeriodicRefresh);
                
                refresh_commands.push(continue_command);
                Command::batch(refresh_commands)
            }
            
            // Chat di gruppo
            Message::StartGroupChat(group_id, group_name) => {
                self.group_chat_target = Some((group_id.clone(), group_name.clone()));
                self.app_state = AppState::GroupChat(group_id, group_name);
                self.group_chat_input.clear();
                Command::none()
            }
            
            Message::CloseGroupChat => {
                self.group_chat_target = None;
                self.app_state = AppState::MainActions;
                self.group_chat_input.clear();
                Command::none()
            }
            
            Message::GroupChatInputChanged(input) => {
                self.group_chat_input = input;
                Command::none()
            }
            
            Message::SendGroupMessage => {
                if let Some((group_id, group_name)) = &self.group_chat_target {
                    if !self.group_chat_input.trim().is_empty() {
                        if let Some(connection) = &self.persistent_connection {
                            let conn = connection.clone();
                            let message = format!("/group {} {}", group_name, self.group_chat_input);
                            let sent_message = format!("You: {}", self.group_chat_input);
                            
                            // Aggiungi il messaggio alla cronologia locale
                            self.group_messages
                                .entry(group_id.clone())
                                .or_insert_with(Vec::new)
                                .push(sent_message);
                            
                            self.group_chat_input.clear();
                            
                            return Command::perform(
                                Self::send_command_persistent(conn, message),
                                |result| match result {
                                    Ok(_) => Message::ShowAlert("Message sent".to_string(), AlertType::Success),
                                    Err(e) => Message::ShowAlert(format!("Error: {}", e), AlertType::Error)
                                }
                            );
                        }
                    }
                }
                Command::none()
            }
            
            // Gestione lista utenti per inviti a gruppi
            Message::ShowUsersListForGroup(group_id) => {
                // Mostra la lista degli utenti per invitare al gruppo
                self.users_list_for_group = Some(group_id);
                self.invite_form_group = None; // Chiudi il form se era aperto
                
                // OTTIMIZZAZIONE: Usa cache se recente (meno di 30 secondi)
                let should_fetch = if let Some(timestamp) = self.users_cache_timestamp {
                    timestamp.elapsed() > std::time::Duration::from_secs(30)
                } else {
                    true
                };
                
                if should_fetch {
                    // Carica la lista degli utenti (tutti gli utenti registrati, escluso l'utente corrente)
                    if matches!(self.connection_state, ConnectionState::Registered) {
                        if let Some(connection) = &self.persistent_connection {
                            let conn = connection.clone();
                            Command::perform(
                                Self::send_command_persistent(conn, "/all_users".to_string()),
                                |result| match result {
                                    Ok(response) => {
                                        let users = Self::parse_users_response(&response);
                                        Message::UsersListUpdated(users)
                                    }
                                    Err(e) => Message::ServerMessage(format!("Error: {}", e))
                                }
                            )
                        } else {
                            Command::none()
                        }
                    } else {
                        Command::none()
                    }
                } else {
                    // Usa cache esistente
                    Command::none()
                }
            }
            
            Message::InviteUserToGroup(username, group_id) => {
                // Invia l'invito direttamente usando l'ID del gruppo
                if matches!(self.connection_state, ConnectionState::Registered) {
                    if let Some(connection) = &self.persistent_connection {
                        // Nascondi la lista degli utenti dopo aver inviato l'invito
                        self.users_list_for_group = None;
                        
                        // Converted to alert system
                        let conn = connection.clone();
                        let command = format!("/invite_by_id {} {}", username, group_id);
                        
                        let alert_command = Command::perform(async {}, move |_| Message::ShowAlert(
                            format!("Inviting {} to group (ID: {})...", username, group_id),
                            AlertType::Info
                        ));
                        
                        let send_command = Command::perform(
                            Self::send_command_persistent(conn, command),
                            |result| match result {
                                Ok(response) => Message::ServerMessage(format!("[Invite] {}", response)),
                                Err(e) => Message::ServerMessage(format!("Error: {}", e))
                            }
                        );
                        
                        Command::batch(vec![alert_command, send_command])
                    } else {
                        // Converted to alert system
                        Command::none()
                    }
                } else {
                    // Converted to alert system
                    Command::none()
                }
            }
            
            Message::HideUsersListForGroup => {
                // Nascondi la lista degli utenti
                self.users_list_for_group = None;
                Command::none()
            }
            
            // Nuovi casi per la gestione degli inviti per gruppo specifico
            Message::InviteToSpecificGroup(group_id) => {
                // Mostra il form di invito per il gruppo specifico (ora usa ID)
                self.invite_form_group = Some(group_id);
                self.invite_to_group_username.clear();
                Command::none()
            }
            
            Message::ShowInviteFormForGroup(group_id) => {
                // Mostra direttamente la lista degli utenti invece del form (ora usa ID)
                self.users_list_for_group = Some(group_id);
                self.invite_form_group = None;
                
                // Carica la lista degli utenti (tutti gli utenti registrati, escluso l'utente corrente)
                if matches!(self.connection_state, ConnectionState::Registered) {
                    if let Some(connection) = &self.persistent_connection {
                        let conn = connection.clone();
                        Command::perform(
                            Self::send_command_persistent(conn, "/all_users".to_string()),
                            |result| match result {
                                Ok(response) => {
                                    let users = Self::parse_users_response(&response);
                                    Message::UsersListUpdated(users)
                                }
                                Err(e) => Message::ServerMessage(format!("Error: {}", e))
                            }
                        )
                    } else {
                        // Converted to alert system
                        Command::none()
                    }
                } else {
                    // Converted to alert system
                    Command::none()
                }
            }
            
            Message::InviteToGroupUsernameChanged(username) => {
                // Aggiorna il campo username per l'invito al gruppo
                self.invite_to_group_username = username;
                Command::none()
            }
            
            Message::SendInviteToGroup => {
                // Invia l'invito al gruppo specifico usando l'ID
                if matches!(self.connection_state, ConnectionState::Registered) {
                    if let Some(group_id) = &self.invite_form_group {
                        let username = self.invite_to_group_username.clone();
                        
                        if !username.is_empty() {
                            if let Some(connection) = &self.persistent_connection {
                                // Converted to alert system
                                let conn = connection.clone();
                                let command = format!("/invite_by_id {} {}", username, group_id);
                                
                                // Chiudi il form dopo aver inviato l'invito
                                self.invite_form_group = None;
                                self.invite_to_group_username.clear();
                                
                                Command::perform(
                                    Self::send_command_persistent(conn, command),
                                    |result| match result {
                                        Ok(response) => Message::ServerMessage(format!("[Invite] {}", response)),
                                        Err(e) => Message::ServerMessage(format!("Error: {}", e))
                                    }
                                )
                            } else {
                                // Converted to alert system
                                Command::none()
                            }
                        } else {
                            // Converted to alert system
                            Command::none()
                        }
                    } else {
                        // Converted to alert system
                        Command::none()
                    }
                } else {
                    // Converted to alert system
                    Command::none()
                }
            }
            
            Message::CancelInviteToGroup => {
                // Cancella il form di invito e la lista utenti
                self.invite_form_group = None;
                self.invite_to_group_username.clear();
                self.users_list_for_group = None;
                Command::none()
            }
            
            Message::ShowAlert(message, alert_type) => {
                self.current_alert = Some(Alert {
                    message,
                    alert_type,
                });
                // Nasconde l'alert automaticamente dopo 3 secondi
                Command::perform(
                    tokio::time::sleep(tokio::time::Duration::from_secs(3)),
                    |_| Message::HideAlert
                )
            }
            
            Message::HideAlert => {
                self.current_alert = None;
                Command::none()
            }

            // Gestione eliminazione messaggi
            Message::DeleteGroupMessages(group_id) => {
                self.delete_confirmation = Some(DeleteConfirmation {
                    delete_type: DeleteType::GroupMessages,
                    target: group_id,
                });
                Command::none()
            }

            Message::DeletePrivateMessages(username) => {
                // Elimina SOLO i messaggi locali, non dal server
                // Questo permette ad ogni utente di eliminare la propria cronologia
                // senza influenzare la cronologia dell'altro utente
                
                // Rimuovi dalla cronologia locale
                self.private_messages.remove(&username);
                
                // Chiudi il menu dopo l'eliminazione
                self.private_chat_menu_open = false;
                
                // Utilizza il manager scalabile per registrare l'eliminazione
                let manager = self.deleted_chats_manager.clone();
                let username_clone = username.clone();
                
                Command::perform(
                    async move {
                        let mut manager_lock = manager.lock().await;
                        let username_for_message = username_clone.clone();
                        println!("🗑️  Deleting chat for user: {}", username_clone);
                        match manager_lock.mark_chat_deleted(username_clone).await {
                            Ok(_) => {
                                println!("✅ Successfully marked chat as deleted for user: {}", username_for_message);
                                format!("Chat with {} successfully deleted", username_for_message)
                            },
                            Err(e) => {
                                println!("❌ Failed to mark chat as deleted for user {}: {}", username_for_message, e);
                                format!("Failed to delete chat with {}: {}", username_for_message, e)
                            },
                        }
                    },
                    |result| Message::ShowAlert(result, AlertType::Success)
                )
            }

            Message::ConfirmDeleteGroupMessages(group_id) => {
                self.delete_confirmation = None;
                if matches!(self.connection_state, ConnectionState::Registered) {
                    if let Some(connection) = &self.persistent_connection {
                        let conn = connection.clone();
                        let command = format!("/delete_group_messages {}", group_id);
                        
                        return Command::perform(
                            Self::send_command_persistent(conn, command),
                            |result| match result {
                                Ok(response) => Message::ShowAlert(
                                    format!("Messages deleted: {}", response),
                                    AlertType::Success
                                ),
                                Err(e) => Message::ShowAlert(
                                    format!("Error deleting messages: {}", e),
                                    AlertType::Error
                                )
                            }
                        );
                    }
                }
                Command::none()
            }

            Message::CancelDelete => {
                self.delete_confirmation = None;
                Command::none()
            }
            
            Message::None => Command::none(),
        }
    }

    fn view(&self) -> Element<Message> {
        match self.app_state {
            AppState::Registration => self.view_registration(),
            AppState::MainActions => self.view_main_actions(),
            AppState::Chat => self.view_main_actions(), // Per ora usa la stessa vista
            AppState::PrivateChat(ref username) => self.view_private_chat(username),
            AppState::GroupChat(ref group_id, ref group_name) => self.view_group_chat(group_id, group_name),
        }
    }
}

impl ChatApp {
    fn view_registration(&self) -> Element<Message> {
        let status_text = match &self.connection_state {
            ConnectionState::Disconnected => "[OFFLINE] Ready to connect",
            ConnectionState::Connecting => "[CONNECTING] Connecting...", 
            ConnectionState::Connected => "[ONLINE] Connected",
            ConnectionState::Registered => "[REGISTERED] Ready for actions",
            ConnectionState::Error(e) => &format!("[ERROR] {}", e),
        };

        // Connection mode selection
        let connection_mode_selection = column![
            text("Choose Connection Mode:").size(18),
            row![
                radio("Localhost", ConnectionMode::Localhost, Some(self.connection_mode), Message::ConnectionModeSelected),
                radio("Remote Host", ConnectionMode::Remote, Some(self.connection_mode), Message::ConnectionModeSelected),
                radio("Manual", ConnectionMode::Manual, Some(self.connection_mode), Message::ConnectionModeSelected),
            ].spacing(10),
        ].spacing(10);

        // Manual connection inputs (only show if Manual is selected)
        let manual_inputs = if matches!(self.connection_mode, ConnectionMode::Manual) {
            column![
                text_input("Host address", &self.manual_host)
                    .on_input(Message::ManualHostChanged)
                    .padding(5),
                text_input("Port", &self.manual_port)
                    .on_input(Message::ManualPortChanged)
                    .padding(5),
            ].spacing(5)
        } else {
            column![]
        };

        let username_input = column![
            text("Enter your username:").size(16),
            text_input("Username", &self.username)
                .on_input(Message::UsernameChanged)
                .padding(8)
                .width(Length::Fixed(300.0)),
        ].spacing(5);

        let register_button = button("Connect & Register")
            .on_press(Message::RegisterPressed)
            .padding(10)
            .width(Length::Fixed(200.0));

        // Sezione Alert (se presente)
        let alert_section = if let Some(ref alert) = self.current_alert {
            let (_alert_color, alert_icon) = match alert.alert_type {
                AlertType::Success => (iced::Color::from_rgb(0.2, 0.8, 0.2), "✅"),
                AlertType::Error => (iced::Color::from_rgb(0.8, 0.2, 0.2), "❌"),
                AlertType::Warning => (iced::Color::from_rgb(0.8, 0.8, 0.2), "⚠️"),
                AlertType::Info => (iced::Color::from_rgb(0.2, 0.6, 0.8), "ℹ️"),
            };
            container(
                text(format!("{} {}", alert_icon, alert.message))
                    .size(14)
            )
            .padding(10)
            .style(iced::theme::Container::Box)
        } else {
            container(text(""))
        };

        let content = column![
            text("Ruggine Chat")
                .size(28)
                .horizontal_alignment(iced::alignment::Horizontal::Center),
            
            text(status_text).size(14),
            
            connection_mode_selection,
            manual_inputs,
            
            username_input,
            register_button,
            
            alert_section,
        ]
        .spacing(15)
        .padding(30)
        .align_items(iced::Alignment::Center);

        container(scrollable(content))
            .width(Length::Fill)
            .height(Length::Fill)
            .center_x()
            .center_y()
            .into()
    }
    
    fn view_main_actions(&self) -> Element<Message> {
        let status_text = match &self.connection_state {
            ConnectionState::Disconnected => "[OFFLINE] Disconnected",
            ConnectionState::Connecting => "[CONNECTING] Connecting...",
            ConnectionState::Connected => "[ONLINE] Connected",
            ConnectionState::Registered => "[REGISTERED] Ready for actions",
            ConnectionState::Error(e) => &format!("[ERROR] {}", e),
        };

        // Header con titolo e disconnect button
        let header = row![
            text("Ruggine Chat - Main Actions")
                .size(24)
                .horizontal_alignment(iced::alignment::Horizontal::Center),
            // Spazio per spingere il pulsante a destra
            text("").width(Length::Fill),
            // Pulsante disconnect rosso in alto a destra
            button(text("🔌 Disconnect").font(EMOJI_FONT))
                .on_press(Message::DisconnectPressed)
                .style(iced::theme::Button::Destructive)
                .padding(10),
        ]
        .align_items(iced::Alignment::Center)
        .width(Length::Fill);

        // Sezione Users
        let users_header = row![
            text("👤").font(EMOJI_FONT).size(20),
            text(" Users").font(BOLD_FONT).size(20),
        ]
        .align_items(iced::Alignment::Center);
        
        let users_button_text = if self.users_expanded { "🔽 Users" } else { "▶️ Users" };
        let mut users_section = column![
            users_header,
            button(text(users_button_text).font(EMOJI_FONT))
                .on_press(Message::ListUsersPressed)
                .width(Length::Fixed(150.0))
                .padding(8),
        ].spacing(5);
        
        // Aggiungi la lista degli utenti con bottoni chat (solo se expanded e non siamo nel contesto di inviti)
        if self.users_expanded && self.users_list_for_group.is_none() && !self.available_users.is_empty() {
            let users_list_section = column![
                row![
                text("🔒").font(EMOJI_FONT).size(14),
                text(" Start Private Chat:").font(BOLD_FONT).size(14),
                ],
                column(
                    self.available_users
                        .iter()
                        .map(|user| {
                            row![
                                text(format!("👤 {}", user)).width(Length::Fill).font(EMOJI_FONT),                                
                                button(text("💬 Chat").font(EMOJI_FONT))
                                    .on_press(Message::StartPrivateChat(user.clone()))
                                    .padding(5)
                                    .width(Length::Fixed(80.0)),
                            ].spacing(10)
                            .align_items(iced::Alignment::Center)
                            .into()
                        })
                        .collect::<Vec<_>>()
                ).spacing(5),
            ].spacing(10);
            users_section = users_section.push(users_list_section);
        }

        // Sezione Groups con toggle
        let groups_section_header = row![
            text("👥").font(EMOJI_FONT).size(20),
            text(" Groups").font(BOLD_FONT).size(20),
        ]
        .align_items(iced::Alignment::Center);
        
        let groups_button_text = if self.groups_expanded { "🔽 Groups" } else { "▶️ Groups" };
        let mut groups_section = column![
            groups_section_header,
            button(text(groups_button_text).font(EMOJI_FONT))
                .on_press(Message::ToggleGroupsSection)
                .width(Length::Fixed(150.0))
                .padding(8),
        ];

        if self.groups_expanded {
            // Pulsanti per sottosezioni
            let groups_submenu = row![
                button(text("📋 List Groups").font(EMOJI_FONT))
                    .on_press(Message::ShowGroupsList)
                    .padding(5),
                button(text("➕ Create Group").font(EMOJI_FONT))
                    .on_press(Message::ShowGroupsCreate)
                    .padding(5),
            ].spacing(10);
            
            groups_section = groups_section.push(groups_submenu);
            
            // Mostra la vista specifica
            if self.groups_list_view {
                // Lista dei miei gruppi
                if !self.my_groups.is_empty() {
                    let groups_list_section = column![
                        text("My Groups:").size(14),
                        column(
                            self.my_groups
                                .iter()
                                .map(|(group_id, group_name)| {
                                    row![
                                        text(format!("👥 {}", group_name)).width(Length::Fill).font(EMOJI_FONT),
                                        button(text("💬").font(EMOJI_FONT))
                                            .on_press(Message::StartGroupChat(group_id.clone(), group_name.clone()))
                                            .padding(5)
                                            .width(Length::Fixed(35.0)),
                                        button(text("➕").font(EMOJI_FONT))
                                            .on_press(Message::ShowInviteFormForGroup(group_id.clone()))
                                            .padding(5)
                                            .width(Length::Fixed(35.0)),
                                        button(text("🗑️").font(EMOJI_FONT))
                                            .on_press(Message::DeleteGroupMessages(group_id.clone()))
                                            .padding(5)
                                            .width(Length::Fixed(35.0)),
                                        button(text("❌ Leave").font(EMOJI_FONT))
                                            .on_press(Message::LeaveSpecificGroup(group_id.clone()))
                                            .padding(5)
                                            .width(Length::Fixed(80.0)),
                                    ].spacing(10)
                                    .align_items(iced::Alignment::Center)
                                    .into()
                                })
                                .collect::<Vec<_>>()
                        ).spacing(5)
                    ].spacing(10);
                    groups_section = groups_section.push(groups_list_section);
                    
                    // Lista utenti per invito a gruppo specifico (se attiva)
                    if let Some(ref selected_group) = self.users_list_for_group {
                        if !self.available_users.is_empty() {
                            let users_list_section = column![
                                text(format!("👥 Invite users to group: {}", selected_group)).size(14).font(BOLD_FONT),
                                column(
                                    self.available_users
                                        .iter()
                                        .map(|user| {
                                            row![
                                                text(format!("👤 {}", user)).width(Length::Fill).font(EMOJI_FONT),
                                                button(text("📤 Invite").font(EMOJI_FONT))
                                                    .on_press(Message::InviteUserToGroup(user.clone(), selected_group.clone()))
                                                    .padding(5)
                                                    .width(Length::Fixed(80.0)),
                                            ].spacing(10)
                                            .align_items(iced::Alignment::Center)
                                            .into()
                                        })
                                        .collect::<Vec<_>>()
                                ).spacing(5),
                                button(text("❌ Cancel").font(EMOJI_FONT))
                                    .on_press(Message::HideUsersListForGroup)
                                    .padding(5),
                            ].spacing(10);
                            groups_section = groups_section.push(users_list_section);
                        } else {
                            let loading_section = column![
                                text(format!("👥 Invite users to group: {}", selected_group)).size(14).font(BOLD_FONT),
                                text("Loading users..."),
                                button(text("❌ Cancel").font(EMOJI_FONT))
                                    .on_press(Message::HideUsersListForGroup)
                                    .padding(5),
                            ].spacing(10);
                            groups_section = groups_section.push(loading_section);
                        }
                    }
                    
                    // Form di invito per gruppo specifico (se attivo) - DEPRECATED, mantenuto per compatibilità
                    if let Some(ref selected_group) = self.invite_form_group {
                        let invite_form = column![
                            text(format!("🎯 Invite user to group: {}", selected_group)).size(14).font(BOLD_FONT),
                            text_input("Enter username to invite", &self.invite_to_group_username)
                                .on_input(Message::InviteToGroupUsernameChanged)
                                .padding(5),
                            row![
                                button(text("✅ Send Invite").font(EMOJI_FONT))
                                    .on_press(Message::SendInviteToGroup)
                                    .padding(5),
                                button(text("❌ Cancel").font(EMOJI_FONT))
                                    .on_press(Message::CancelInviteToGroup)
                                    .padding(5),
                            ].spacing(10),
                        ].spacing(5);
                        groups_section = groups_section.push(invite_form);
                    }
                } else {
                    groups_section = groups_section.push(text("No groups found or loading..."));
                }
            } else if self.groups_create_view {
                // Form per creare gruppo
                let create_group_form = column![
                    text("Create New Group:").size(14),
                    text_input("Enter group name", &self.group_name)
                        .on_input(Message::GroupNameChanged)
                        .padding(5),
                    row![
                        button(text("✅ Create Group").font(EMOJI_FONT))
                            .on_press(Message::CreateGroupPressed)
                            .padding(5),
                        button(text("❌ Cancel").font(EMOJI_FONT))
                            .on_press(Message::HideGroupsSubsections)
                            .padding(5),
                    ].spacing(10),
                ].spacing(5);
                groups_section = groups_section.push(create_group_form);
            }
        }

        // Sezione Invites con toggle
        let invites_section_header = row![
            text("📮").font(EMOJI_FONT).size(20),
            text(" Invites").font(BOLD_FONT).size(20),
        ]
        .align_items(iced::Alignment::Center);
        
        let invites_button_text = if self.invites_expanded { "🔽 Invites" } else { "▶️ Invites" };
        let mut invites_section = column![
            invites_section_header,
            button(text(invites_button_text).font(EMOJI_FONT))
                .on_press(Message::ToggleInvitesSection)
                .width(Length::Fixed(150.0))
                .padding(8),
        ];

        if self.invites_expanded {
            // Mostra direttamente la lista degli inviti ricevuti
            if !self.my_invites.is_empty() {
                let invites_list_section = column![
                    text("Received Invites:").size(14),
                    column(
                        self.my_invites
                            .iter()
                            .map(|(invite_id, group_name)| {
                                row![
                                    text(format!("📩 Group: {} (ID: {})", group_name, invite_id)).width(Length::Fill).font(EMOJI_FONT),
                                    button(text("✅ Accept").font(EMOJI_FONT))
                                        .on_press(Message::AcceptSpecificInvite(invite_id.clone()))
                                        .padding(5)
                                        .width(Length::Fixed(80.0)),
                                    button(text("❌ Reject").font(EMOJI_FONT))
                                        .on_press(Message::RejectSpecificInvite(invite_id.clone()))
                                        .padding(5)
                                        .width(Length::Fixed(80.0)),
                                ].spacing(10)
                                .align_items(iced::Alignment::Center)
                                .into()
                            })
                            .collect::<Vec<_>>()
                    ).spacing(5)
                ].spacing(10);
                invites_section = invites_section.push(invites_list_section);
            } else {
                invites_section = invites_section.push(text("No pending invites or loading..."));
            }
        }

        // Sezione Alert (se presente)
        let alert_section = if let Some(ref alert) = self.current_alert {
            let (_alert_color, alert_icon) = match alert.alert_type {
                AlertType::Success => (iced::Color::from_rgb(0.2, 0.8, 0.2), "✅"),
                AlertType::Error => (iced::Color::from_rgb(0.8, 0.2, 0.2), "❌"),
                AlertType::Warning => (iced::Color::from_rgb(0.8, 0.8, 0.2), "⚠️"),
                AlertType::Info => (iced::Color::from_rgb(0.2, 0.6, 0.8), "ℹ️"),
            };
            container(
                text(format!("{} {}", alert_icon, alert.message))
                    .size(14)
            )
            .padding(10)
            .style(iced::theme::Container::Box)
        } else {
            container(text(""))
        };

        let content = column![
            header,
            
            text(status_text).size(16),
            text(format!("Logged in as: {}", self.username)).size(14),
            
            alert_section,
            
            users_section,
            groups_section,
            invites_section,
        ]
        .spacing(15)
        .padding(20);

        container(scrollable(content))
            .width(Length::Fill)
            .height(Length::Fill)
            .center_x()
            .into()
    }
    
    fn get_connection_info(&self) -> Option<(String, String)> {
        match &self.connection_mode {
            ConnectionMode::Localhost => Some(("127.0.0.1".to_string(), "5000".to_string())),
            ConnectionMode::Remote => {
                if let Some(config) = &self.config {
                    Some((config.public_host.clone(), config.default_port.to_string()))
                } else {
                    None
                }
            }
            ConnectionMode::Manual => {
                if !self.manual_host.is_empty() && !self.manual_port.is_empty() {
                    Some((self.manual_host.clone(), self.manual_port.clone()))
                } else {
                    None
                }
            }
        }
    }
    
    async fn connect_and_register_persistent(
        host: String, 
        port: String, 
        username: String
    ) -> Result<(String, Arc<Mutex<PersistentConnection>>), Box<dyn std::error::Error + Send + Sync>> {
        let address = format!("{}:{}", host, port);
        
        println!("Connecting to {}", address);
        
        // Connetti al server
        let stream = TcpStream::connect(&address).await?;
        
        // Crea reader e writer (owned)
        let (reader, writer) = stream.into_split();
        let mut writer = BufWriter::new(writer);
        let mut reader = BufReader::new(reader);
        
        println!("Connected, reading welcome message...");
        
        // Leggi il messaggio di benvenuto completo - il server invia esattamente 8 righe
        let mut welcome_lines = Vec::new();
        for i in 0..8 {
            let mut line = String::new();
            match reader.read_line(&mut line).await {
                Ok(0) => {
                    println!("EOF raggiunto dopo {} righe", i);
                    break; // EOF
                }
                Ok(_) => {
                    let trimmed = line.trim();
                    println!("Welcome line {}: '{}'", i, trimmed);
                    welcome_lines.push(trimmed.to_string());
                }
                Err(e) => {
                    println!("Errore durante lettura welcome: {}", e);
                    return Err(e.into());
                }
            }
        }
        
        println!("Welcome message read, sending registration...");
        
        // Invia il comando di registrazione
        let command = format!("/register {}", username);
        writer.write_all(command.as_bytes()).await?;
        writer.write_all(b"\n").await?;
        writer.flush().await?;
        
        println!("Registration sent, waiting for response...");
        
        // Leggi la risposta
        let mut response = String::new();
        reader.read_line(&mut response).await?;
        
        println!("Registration response: {}", response.trim());
        
        // Crea la connessione persistente
        let persistent_conn = PersistentConnection {
            writer,
            reader,
        };
        
        Ok((response.trim().to_string(), Arc::new(Mutex::new(persistent_conn))))
    }
    
    async fn send_command_persistent(
        connection: Arc<Mutex<PersistentConnection>>, 
        command: String
    ) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        // OTTIMIZZATO: Timeout aumentato e attesa invece di errore
        let mut conn = match tokio::time::timeout(
            tokio::time::Duration::from_millis(2000), 
            connection.lock()
        ).await {
            Ok(conn) => conn,
            Err(_) => {
                // Invece di restituire errore, attendi di più
                tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                connection.lock().await
            },
        };
        
        println!("Sending command: {}", command);
        
        // Invia il comando
        conn.writer.write_all(command.as_bytes()).await?;
        conn.writer.write_all(b"\n").await?;
        conn.writer.flush().await?;
        
        // OTTIMIZZATO: Timeout ridotto per lettura più veloce (da 2s a 1s)
        let mut full_response = String::new();
        let mut first_line = String::new();
        
        match tokio::time::timeout(
            tokio::time::Duration::from_millis(1000), 
            conn.reader.read_line(&mut first_line)
        ).await {
            Ok(Ok(_)) => {},
            Ok(Err(e)) => return Err(format!("Read error: {}", e).into()),
            Err(_) => return Err("Server response timeout".into()),
        }
        
        println!("First line received: {}", first_line.trim());
        
        // Controlla se è una notifica (e la ignora per ora)
        if first_line.trim().starts_with("NOTIFICATION:") {
            println!("Received notification, reading actual response...");
            // Questa è una notifica, leggi la vera risposta al comando
            match tokio::time::timeout(
                tokio::time::Duration::from_millis(500), // OTTIMIZZATO: Ridotto da 1s a 500ms
                conn.reader.read_line(&mut first_line)
            ).await {
                Ok(Ok(_)) => {},
                Ok(Err(e)) => return Err(format!("Read error after notification: {}", e).into()),
                Err(_) => return Err("Server response timeout after notification".into()),
            }
            println!("Actual response line: {}", first_line.trim());
        }
        
        full_response.push_str(&first_line);
        
        // Se la prima riga contiene "Your pending invites:" o altri pattern multi-riga,
        // continua a leggere le righe successive
        if first_line.contains("Your pending invites:") || 
           first_line.contains("Your groups:") ||
           first_line.contains("Online users:") ||
           first_line.contains("All users:") ||
           first_line.contains("Private messages:") {
            
            println!("Multi-line response detected, reading additional lines...");
            
            // OTTIMIZZATO: Timeout ridotto per righe aggiuntive (da 200ms a 150ms)
            loop {
                let mut line = String::new();
                match tokio::time::timeout(tokio::time::Duration::from_millis(150), conn.reader.read_line(&mut line)).await {
                    Ok(Ok(0)) => break, // Connessione chiusa
                    Ok(Ok(_)) => {
                        println!("Additional line: {}", line.trim());
                        // Ignora le notifiche anche qui
                        if line.trim().starts_with("NOTIFICATION:") {
                            continue;
                        }
                        if line.trim().is_empty() {
                            break; // Riga vuota indica fine della risposta multi-riga
                        }
                        full_response.push_str(&line);
                    }
                    _ => break, // Timeout o errore - fine della risposta
                }
            }
        }
        
        println!("Final response: {}", full_response.trim());
        Ok(full_response.trim().to_string())
    }
    
    // Helper functions per parsare le risposte del server
    fn parse_groups_response(response: &str) -> Vec<(String, String)> {
        // Nuovo formato della risposta:
        // "OK: Your groups:\n  ID: uuid1 | Name: 'gruppo1'\n  ID: uuid2 | Name: 'gruppo2'"
        // oppure
        // "OK: You are not in any groups"
        if response.starts_with("OK:") {
            if response.contains("You are not in any groups") {
                return Vec::new();
            } else if response.contains("Your groups:") {
                let mut groups = Vec::new();
                
                // Dividi per righe e cerca quelle che iniziano con "  ID:"
                for line in response.lines() {
                    let trimmed = line.trim();
                    if trimmed.starts_with("ID:") {
                        // Parse: "ID: uuid | Name: 'group_name'"
                        let parts: Vec<&str> = trimmed.split(" | ").collect();
                        if parts.len() == 2 {
                            if let Some(id_part) = parts[0].strip_prefix("ID: ") {
                                if let Some(name_part) = parts[1].strip_prefix("Name: '").and_then(|s| s.strip_suffix("'")) {
                                    groups.push((id_part.to_string(), name_part.to_string()));
                                }
                            }
                        }
                    }
                }
                
                groups
            } else {
                Vec::new()
            }
        } else {
            Vec::new()
        }
    }
    
    fn parse_invites_response(response: &str) -> Vec<(String, String)> {
        // Esempi di risposta:
        // - "OK: You have no pending invites"
        // - "OK: Your pending invites:\n  ID: uuid | Group: 'gruppo1' | From: username | Date: 2025-01-01 12:00"
        
        if response.starts_with("OK:") {
            if response.contains("You have no pending invites") {
                return Vec::new();
            } else if response.contains("Your pending invites:") {
                // Estrai le righe degli inviti (saltando la prima parte)
                let lines: Vec<&str> = response.lines().collect();
                let mut invites = Vec::new();
                
                for line in lines.iter().skip(1) { // Salta la prima riga "OK: Your pending invites:"
                    let line = line.trim(); // Rimuove spazi iniziali e finali
                    
                    if line.is_empty() {
                        continue;
                    }
                    
                    // Parse formato: "ID: uuid | Group: 'gruppo1' | From: username | Date: 2025-01-01 12:00"
                    if line.starts_with("ID: ") {
                        let parts: Vec<&str> = line.split(" | ").collect();
                        
                        if let Some(id_part) = parts.get(0) {
                            if let Some(invite_id) = id_part.strip_prefix("ID: ") {
                                // Cerca la parte del gruppo
                                if let Some(group_part) = parts.get(1) {
                                    if let Some(group_name) = group_part.strip_prefix("Group: '").and_then(|s| s.strip_suffix("'")) {
                                        invites.push((invite_id.to_string(), group_name.to_string()));
                                    }
                                }
                            }
                        }
                    }
                }
                invites
            } else {
                Vec::new()
            }
        } else {
            Vec::new()
        }
    }
    
    fn parse_users_response(response: &str) -> Vec<String> {
        // Esempi di risposta:
        // - "OK: Online users: user1, user2, user3"
        // - "OK: All users: user1, user2, user3"
        // - "OK: No users online"
        if response.starts_with("OK:") {
            if response.contains("No users online") {
                return Vec::new();
            } else if response.contains("Online users:") {
                let users_part = response.split("Online users:").nth(1).unwrap_or("").trim();
                if users_part.is_empty() {
                    Vec::new()
                } else {
                    users_part
                        .split(',')
                        .map(|s| s.trim().to_string())
                        .filter(|s| !s.is_empty())
                        .collect()
                }
            } else if response.contains("All users:") {
                let users_part = response.split("All users:").nth(1).unwrap_or("").trim();
                if users_part.is_empty() {
                    Vec::new()
                } else {
                    users_part
                        .split(',')
                        .map(|s| s.trim().to_string())
                        .filter(|s| !s.is_empty())
                        .collect()
                }
            } else {
                Vec::new()
            }
        } else {
            Vec::new()
        }
    }
    
    fn parse_private_messages_response(
        response: &str, 
        current_username: &str,
        current_user_uuid: Option<&str>,
        pending_messages: &mut Vec<String>
    ) -> Vec<(String, bool)> {
        // Ritorna una tupla (messaggio, is_sent_by_me)
        println!("DEBUG: Parsing private messages with current_username: '{}', current_user_uuid: {:?}", current_username, current_user_uuid);
        println!("DEBUG: Server response: '{}'", response);
        println!("DEBUG: Pending messages: {:?}", pending_messages);
        
        if response.starts_with("OK:") {
            let mut messages = Vec::new();
            
            // Gestisci diversi formati di risposta possibili
            if response.contains("Private messages:") {
                // Formato: "OK: Private messages:\nMessaggio 1\nMessaggio 2\n..."
                let lines: Vec<&str> = response.lines().collect();
                println!("DEBUG: Found {} lines in response", lines.len());
                
                if lines.len() > 1 {
                    // Salta la prima riga ("OK: Private messages:")
                    for (i, line) in lines[1..].iter().enumerate() {
                        if !line.trim().is_empty() {
                            println!("DEBUG: Processing line {}: '{}'", i, line.trim());
                            let (cleaned_msg, is_sent_by_me) = Self::parse_message_line(line.trim(), current_username, current_user_uuid, pending_messages);
                            if !cleaned_msg.is_empty() {
                                messages.push((cleaned_msg, is_sent_by_me));
                            }
                        }
                    }
                }
            } else if response.contains("Messages:") {
                // Formato alternativo: "OK: Messages:\n..."
                let lines: Vec<&str> = response.lines().collect();
                if lines.len() > 1 {
                    for line in lines[1..].iter() {
                        if !line.trim().is_empty() {
                            let (cleaned_msg, is_sent_by_me) = Self::parse_message_line(line.trim(), current_username, current_user_uuid, pending_messages);
                            if !cleaned_msg.is_empty() {
                                messages.push((cleaned_msg, is_sent_by_me));
                            }
                        }
                    }
                }
            } else {
                // Formato semplice: ogni riga dopo "OK:" è un messaggio
                let lines: Vec<&str> = response.lines().collect();
                if lines.len() > 1 {
                    for line in lines[1..].iter() {
                        if !line.trim().is_empty() && !line.trim().starts_with("OK:") {
                            let (cleaned_msg, is_sent_by_me) = Self::parse_message_line(line.trim(), current_username, current_user_uuid, pending_messages);
                            if !cleaned_msg.is_empty() {
                                messages.push((cleaned_msg, is_sent_by_me));
                            }
                        }
                    }
                }
            }
            
            println!("DEBUG: Final parsed messages: {:?}", messages);
            messages
        } else {
            Vec::new()
        }
    }
    
    fn parse_private_messages_response_with_filter(
        response: &str, 
        current_username: &str,
        current_user_uuid: Option<&str>,
        pending_messages: &mut Vec<String>,
        deletion_timestamp: Option<&std::time::SystemTime>
    ) -> Vec<(String, bool)> {
        // Se l'utente ha eliminato la chat, mostra SOLO i messaggi inviati DOPO il timestamp di eliminazione
        // Tutti i messaggi precedenti al momento dell'eliminazione rimangono nascosti per sempre
        
        // DEBUG: Timestamp di eliminazione con maggiore precisione
        let deletion_timestamp_ms = deletion_timestamp.map(|ts| {
            ts.duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis()
        });
        
        println!("DEBUG: ========== PARSE WITH FILTER START ==========");
        println!("DEBUG: Current username: '{}'", current_username);
        println!("DEBUG: Current user UUID: {:?}", current_user_uuid);
        println!("DEBUG: Deletion timestamp: {:?}", deletion_timestamp);
        println!("DEBUG: Deletion timestamp (ms since epoch): {:?}", deletion_timestamp_ms);
        println!("DEBUG: Pending messages: {:?}", pending_messages);
        println!("DEBUG: Server response length: {} chars", response.len());
        
        if response.starts_with("OK:") {
            let mut messages = Vec::new();
            
            // Se l'utente ha eliminato la chat, filtra i messaggi per timestamp
            let chat_was_deleted = deletion_timestamp.is_some();
            
            println!("DEBUG: Chat was deleted: {}", chat_was_deleted);
            if chat_was_deleted {
                println!("DEBUG: Will filter messages by deletion timestamp");
            }
            
            // Gestisci diversi formati di risposta possibili
            if response.contains("Private messages:") {
                // Formato: "OK: Private messages:\nMessaggio 1\nMessaggio 2\n..."
                let lines: Vec<&str> = response.lines().collect();
                println!("DEBUG: Found {} lines in response", lines.len());
                
                if lines.len() > 1 {
                    // Salta la prima riga ("OK: Private messages:")
                    for (i, line) in lines[1..].iter().enumerate() {
                        if !line.trim().is_empty() {
                            println!("DEBUG: --------- Processing line {} ---------", i);
                            println!("DEBUG: Raw line: '{}'", line.trim());
                            
                            let (cleaned_msg, is_sent_by_me) = Self::parse_message_line(line.trim(), current_username, current_user_uuid, pending_messages);
                            if !cleaned_msg.is_empty() {
                                println!("DEBUG: Parsed message: '{}', is_sent_by_me: {}", cleaned_msg, is_sent_by_me);
                                
                                // Se la chat è stata eliminata, usa una logica semplice:
                                // Mostra solo i messaggi che sono nei pending_messages (inviati dopo l'eliminazione)
                                // O i messaggi molto recenti (approssimazione per quelli ricevuti dopo l'eliminazione)
                                if chat_was_deleted {
                                    let should_show = if pending_messages.contains(&cleaned_msg) {
                                        // I messaggi nei pending sono sempre mostrati (appena inviati)
                                        println!("DEBUG: ✅ Message '{}' is in pending - SHOWING", cleaned_msg);
                                        true
                                    } else {
                                        // Per gli altri messaggi, usa il principio: se la chat è stata eliminata
                                        // molto tempo fa, probabilmente questo messaggio è nuovo
                                        // Logica semplificata: dopo l'eliminazione, accetta nuovi messaggi
                                        // basandoci sul fatto che sono tra gli ultimi nella lista
                                        let total_lines = lines.len() - 1; // -1 per escludere la prima riga "OK:"
                                        let is_among_recent = i >= total_lines.saturating_sub(5); // Solo ultimi 5 messaggi
                                        
                                        println!("DEBUG: Message '{}' at position {} of {} total lines", cleaned_msg, i, total_lines);
                                        println!("DEBUG: is_among_recent (last 5): {}", is_among_recent);
                                        
                                        // NUOVO: Prova a estrarre il timestamp dal messaggio per un confronto più preciso
                                        if let Some(timestamp_ms) = Self::extract_message_timestamp_ms(line.trim()) {
                                            println!("DEBUG: Message timestamp: {} ms", timestamp_ms);
                                            if let Some(deletion_ms) = deletion_timestamp_ms {
                                                let is_after_deletion = timestamp_ms > deletion_ms;
                                                println!("DEBUG: Deletion timestamp: {} ms", deletion_ms);
                                                println!("DEBUG: Message is after deletion: {}", is_after_deletion);
                                                is_after_deletion
                                            } else {
                                                println!("DEBUG: No deletion timestamp available, using position-based logic");
                                                is_among_recent
                                            }
                                        } else {
                                            println!("DEBUG: Could not extract timestamp, using position-based logic");
                                            is_among_recent
                                        }
                                    };
                                    
                                    if should_show {
                                        messages.push((cleaned_msg.clone(), is_sent_by_me));
                                        println!("DEBUG: ✅ ADDED post-deletion message: '{}'", cleaned_msg);
                                    } else {
                                        println!("DEBUG: ❌ SKIPPED pre-deletion message: '{}'", cleaned_msg);
                                    }
                                } else {
                                    // Chat non eliminata - mostra tutto
                                    let msg_for_debug = cleaned_msg.clone();
                                    messages.push((cleaned_msg, is_sent_by_me));
                                    println!("DEBUG: ✅ ADDED message (no deletion): '{}'", msg_for_debug);
                                }
                            } else {
                                println!("DEBUG: ⚠️ Empty message after parsing, skipping");
                            }
                        }
                    }
                }
            } else if response.len() > 3 {
                // Formato diretto: "OK:\nMessaggio 1\nMessaggio 2\n..."
                let content = &response[3..]; // Rimuovi "OK:"
                let lines: Vec<&str> = content.lines().collect();
                
                for (i, line) in lines.iter().enumerate() {
                    if !line.trim().is_empty() {
                        println!("DEBUG: Processing direct line {}: '{}'", i, line.trim());
                        
                        let (cleaned_msg, is_sent_by_me) = Self::parse_message_line(line.trim(), current_username, current_user_uuid, pending_messages);
                        if !cleaned_msg.is_empty() {
                            // Se la chat è stata eliminata, applica lo stesso filtro
                            if chat_was_deleted {
                                let should_show = if pending_messages.contains(&cleaned_msg) {
                                    println!("DEBUG: Message '{}' is in pending - showing", cleaned_msg);
                                    true
                                } else {
                                    let total_lines = lines.len();
                                    let is_among_recent = i >= total_lines.saturating_sub(5); // Solo ultimi 5 messaggi
                                    
                                    println!("DEBUG: Message '{}' at line {} of {} - is_among_recent: {}", 
                                           cleaned_msg, i, total_lines, is_among_recent);
                                    
                                    is_among_recent
                                };
                                
                                if should_show {
                                    messages.push((cleaned_msg.clone(), is_sent_by_me));
                                    println!("DEBUG: Added post-deletion message: '{}'", cleaned_msg);
                                } else {
                                    println!("DEBUG: Skipping pre-deletion message: '{}'", cleaned_msg);
                                }
                            } else {
                                let msg_for_debug = cleaned_msg.clone();
                                messages.push((cleaned_msg, is_sent_by_me));
                                println!("DEBUG: Added message (no deletion): '{}'", msg_for_debug);
                            }
                        }
                    }
                }
            }
            
            println!("DEBUG: ========== PARSE WITH FILTER END ==========");
            println!("DEBUG: Final filtered messages: {:?}", messages);
            messages
        } else {
            println!("DEBUG: Response does not start with 'OK:', returning empty");
            Vec::new()
        }
    }
    
    // Nuova funzione per estrarre timestamp con precisione al millisecondo
    fn extract_message_timestamp_ms(message_line: &str) -> Option<u128> {
        // Il formato è: [HH:MM:SS] uuid: message
        // Proveremo a convertire HH:MM:SS in millisecondo del giorno corrente
        if message_line.starts_with('[') {
            if let Some(close_bracket) = message_line.find(']') {
                let timestamp_str = &message_line[1..close_bracket];
                println!("DEBUG: Extracted timestamp string: '{}'", timestamp_str);
                
                // Parse HH:MM:SS
                let parts: Vec<&str> = timestamp_str.split(':').collect();
                if parts.len() == 3 {
                    if let (Ok(hours), Ok(minutes), Ok(seconds)) = (
                        parts[0].parse::<u64>(),
                        parts[1].parse::<u64>(),
                        parts[2].parse::<u64>()
                    ) {
                        // Converti in millisecondi dal inizio del giorno
                        let total_ms = (hours * 3600 + minutes * 60 + seconds) * 1000;
                        
                        // AGGIORNA: Usa il timestamp Unix attuale come base e aggiungi i millisecondi del giorno
                        let now = std::time::SystemTime::now();
                        let unix_epoch_ms = now.duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_millis();
                        
                        // Calcola l'inizio del giorno corrente (approssimativo)
                        let ms_per_day = 24 * 60 * 60 * 1000;
                        let start_of_day_ms = (unix_epoch_ms / ms_per_day) * ms_per_day;
                        let message_timestamp_ms = start_of_day_ms + total_ms as u128;
                        
                        println!("DEBUG: Converted timestamp: {}:{}:{} -> {} ms since epoch", 
                               hours, minutes, seconds, message_timestamp_ms);
                        
                        return Some(message_timestamp_ms);
                    }
                }
            }
        }
        println!("DEBUG: Could not extract timestamp from: '{}'", message_line);
        None
    }
    
    fn parse_message_line(
        message: &str, 
        current_username: &str,
        current_user_uuid: Option<&str>,
        pending_messages: &mut Vec<String>
    ) -> (String, bool) {
        // Analizza una riga di messaggio per determinare se è stato inviato dall'utente corrente
        println!("DEBUG: ===== PARSE MESSAGE LINE START =====");
        println!("DEBUG: Input message: '{}'", message);
        println!("DEBUG: Current username: '{}'", current_username);
        println!("DEBUG: Current user UUID: {:?}", current_user_uuid);
        println!("DEBUG: Pending messages: {:?}", pending_messages);
        
        // Il formato dei messaggi dal server è: [timestamp] user_id_or_username: message
        // Esempio: [16:07:16] 6b85bc68-acc8-4eae-8506-99c73770d624: jjjj
        // oppure: [16:07:16] username: message
        
        // Prima rimuovi il timestamp se presente
        let message_without_timestamp = if message.starts_with('[') {
            if let Some(close_bracket) = message.find(']') {
                let result = message[close_bracket + 1..].trim().to_string();
                println!("DEBUG: Removed timestamp, result: '{}'", result);
                result
            } else {
                println!("DEBUG: No closing bracket found for timestamp");
                message.to_string()
            }
        } else {
            println!("DEBUG: No timestamp bracket found");
            message.to_string()
        };
        
        if let Some(colon_pos) = message_without_timestamp.find(": ") {
            let sender = message_without_timestamp[..colon_pos].trim();
            let content = &message_without_timestamp[colon_pos + 2..];
            
            println!("DEBUG: Extracted sender: '{}'", sender);
            println!("DEBUG: Extracted content: '{}'", content);
            
            // Prima strategia: controlla se questo messaggio è stato inviato da noi (è nella lista pending)
            let is_sent_by_me = if let Some(index) = pending_messages.iter().position(|msg| msg == content) {
                // Rimuovi dalla lista pending perché l'abbiamo ricevuto dal server
                pending_messages.remove(index);
                println!("DEBUG: ✅ Message '{}' found in pending messages at index {} - MARKING AS SENT BY ME", content, index);
                true
            } else {
                println!("DEBUG: ❌ Message '{}' NOT found in pending messages", content);
                
                // Strategia fallback: controlla se il mittente è l'utente corrente
                let fallback_check = if let Some(uuid) = current_user_uuid {
                    let uuid_match = sender == uuid;
                    println!("DEBUG: UUID comparison: sender '{}' == current_uuid '{}' -> {}", sender, uuid, uuid_match);
                    uuid_match
                } else if sender.len() > 20 && sender.contains('-') {
                    // Sembra un UUID ma non abbiamo l'UUID dell'utente corrente
                    println!("DEBUG: Sender looks like UUID but no current_user_uuid available - assuming NOT sent by me");
                    false
                } else {
                    // Confronto normale con username
                    let username_match = sender == current_username || sender == "You";
                    println!("DEBUG: Username comparison: sender '{}' == current_username '{}' OR 'You' -> {}", sender, current_username, username_match);
                    username_match
                };
                
                println!("DEBUG: Fallback check result: {}", fallback_check);
                fallback_check
            };
            
            println!("DEBUG: FINAL RESULT - sender: '{}', content: '{}', is_sent_by_me: {}", sender, content, is_sent_by_me);
            println!("DEBUG: ===== PARSE MESSAGE LINE END =====");
            
            (content.to_string(), is_sent_by_me)
        } else {
            // Se non c'è il formato "sender: message", considera come messaggio ricevuto
            println!("DEBUG: No colon found in message, treating as received message");
            println!("DEBUG: ===== PARSE MESSAGE LINE END =====");
            (message_without_timestamp, false)
        }
    }
    
    fn clean_message_format(message: &str) -> String {
        // Rimuovi prefissi come "user123: " o "You: " per mostrare solo il contenuto
        if let Some(colon_pos) = message.find(": ") {
            message[colon_pos + 2..].to_string()
        } else {
            message.to_string()
        }
    }
    
    fn view_private_chat(&self, username: &str) -> Element<Message> {
        use iced::widget::{button, column, container, row, text, text_input, scrollable};
        use iced::{Alignment, Element, Length, Color};
        
        let empty_messages = Vec::new();
        let messages = self.private_messages.get(username).unwrap_or(&empty_messages);
        
        // Header con titolo e menu
        let header = row![
            // Titolo della chat
            text(format!("💬 Private chat with {}", username)).size(18).font(BOLD_FONT),
            // Spacer per spingere il menu a destra
            container(text("")).width(Length::Fill),
            // Menu a tendina in alto a destra
            if self.private_chat_menu_open {
                column![
                    button(text("⚙️").font(EMOJI_FONT))
                        .on_press(Message::TogglePrivateChatMenu)
                        .padding(5),
                    container(
                        column![
                            button(text("🗑️ Delete My Chat").font(EMOJI_FONT))
                                .on_press(Message::DeletePrivateMessages(username.to_string()))
                                .padding(8)
                                .width(Length::Fixed(150.0))
                                .style(iced::theme::Button::Destructive),
                        ]
                        .spacing(5)
                    )
                    .padding(5)
                    .style(iced::theme::Container::Box)
                ]
                .align_items(Alignment::End)
            } else {
                column![
                    button(text("⚙️").font(EMOJI_FONT))
                        .on_press(Message::TogglePrivateChatMenu)
                        .padding(5),
                ]
                .align_items(Alignment::End)
            }
        ]
        .spacing(10)
        .align_items(Alignment::Center);
        
        let messages_area = if messages.is_empty() {
            scrollable(
                column![
                    text("No messages yet. Start the conversation!").size(14),
                ].spacing(10)
            ).height(Length::Fixed(300.0))
        } else {
            println!("Rendering {} messages for {}", messages.len(), username);
            
            let message_widgets: Vec<Element<Message>> = messages
                .iter()
                .enumerate()
                .filter(|(_, (msg, _))| {
                    // OTTIMIZZAZIONE: Filtra i messaggi di UUID discovery dalla visualizzazione UI
                    let should_show = !msg.starts_with("__UUID_DISCOVERY_");
                    if !should_show {
                        println!("🚫 Filtering out UUID discovery message from UI: {}", msg);
                    }
                    should_show
                })
                .map(|(i, (msg, is_sent_by_me))| {
                    println!("✅ Rendering UI message {}: '{}', is_sent_by_me: {}", i, msg, is_sent_by_me);
                    
                    if *is_sent_by_me {
                        // Messaggio inviato da te - allineato a destra con colore verde
                        container(
                            text(msg)
                                .style(Color::from_rgb(0.0, 0.7, 0.0)) // Verde
                                .size(14)
                        )
                        .padding(8)
                        .width(Length::Fill)
                        .style(iced::theme::Container::Box)
                        .align_x(iced::alignment::Horizontal::Right)
                        .into()
                    } else {
                        // Messaggio ricevuto - allineato a sinistra con colore blu
                        container(
                            text(msg)
                                .style(Color::from_rgb(0.0, 0.5, 1.0)) // Blu
                                .size(14)
                        )
                        .padding(8)
                        .width(Length::Fill)
                        .style(iced::theme::Container::Box)
                        .align_x(iced::alignment::Horizontal::Left)
                        .into()
                    }
                })
                .collect();
                
            scrollable(
                column(message_widgets)
                    .spacing(5)
                    .padding(10)
            ).height(Length::Fixed(300.0))
        };
        
        let input_section = row![
            text_input("Type your message...", &self.private_chat_input)
                .on_input(Message::PrivateChatInputChanged)
                .on_submit(Message::SendPrivateMessage)
                .padding(5)
                .width(Length::Fill),
            button(text("📤 Send").font(EMOJI_FONT))
                .on_press(Message::SendPrivateMessage)
                .padding(5),
        ].spacing(10).align_items(Alignment::Center);
        
        let back_button = button(text("⬅️ Back to Main").font(EMOJI_FONT))
            .on_press(Message::ClosePrivateChat)
            .padding(5);

        // Layout semplice senza form di conferma
        let content = column![
            back_button,
            header,
            messages_area,
            input_section,
        ].spacing(20).padding(20);
        
        container(content)
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }
    
    fn view_group_chat(&self, group_id: &str, group_name: &str) -> Element<Message> {
        use iced::widget::{button, column, container, row, text, text_input, scrollable};
        use iced::{Alignment, Element, Length, Color};
        
        let empty_messages = Vec::new();
        let messages = self.group_messages.get(group_id).unwrap_or(&empty_messages);
        
        let messages_area = if messages.is_empty() {
            column![
                text(format!("👥 Group chat: {}", group_name)).size(18).font(BOLD_FONT),
                text("No messages yet. Start the conversation!").size(14),
            ].spacing(10)
        } else {
            let message_widgets: Vec<Element<Message>> = messages
                .iter()
                .map(|msg| {
                    let color = if msg.starts_with("You: ") {
                        Color::from_rgb(0.0, 0.7, 0.0) // Verde per i tuoi messaggi
                    } else {
                        Color::from_rgb(0.0, 0.5, 1.0) // Blu per i messaggi degli altri
                    };
                    text(msg).style(color).size(14).into()
                })
                .collect();
                
            column![
                text(format!("👥 Group chat: {}", group_name)).size(18).font(BOLD_FONT),
                scrollable(
                    column(message_widgets)
                        .spacing(5)
                        .padding(10)
                ).height(Length::Fixed(300.0))
            ].spacing(10)
        };
        
        let input_section = row![
            text_input("Type your message...", &self.group_chat_input)
                .on_input(Message::GroupChatInputChanged)
                .on_submit(Message::SendGroupMessage)
                .padding(5)
                .width(Length::Fill),
            button(text("📤 Send").font(EMOJI_FONT))
                .on_press(Message::SendGroupMessage)
                .padding(5),
        ].spacing(10).align_items(Alignment::Center);
        
        let back_button = button(text("⬅️ Back to Main").font(EMOJI_FONT))
            .on_press(Message::CloseGroupChat)
            .padding(5);

        // Menu a tendina per le azioni della chat
        let chat_actions = row![
            button(text("🗑️ Delete Messages").font(EMOJI_FONT))
                .on_press(Message::DeleteGroupMessages(group_id.to_string()))
                .padding(5)
                .style(iced::theme::Button::Destructive),
        ].spacing(10);

        // Controllo se c'è una finestra di conferma di eliminazione
        let content = if let Some(ref confirmation) = self.delete_confirmation {
            if matches!(confirmation.delete_type, DeleteType::GroupMessages) && confirmation.target == group_id {
                // Mostra dialog di conferma
                column![
                    back_button,
                    messages_area,
                    input_section,
                    chat_actions,
                    container(
                        column![
                            text("⚠️ Delete All Messages").size(18).font(BOLD_FONT),
                            text(format!("Are you sure you want to delete all messages in group '{}'?", group_name)).size(14),
                            text("This action cannot be undone.").size(12),
                            row![
                                button(text("✅ Yes, Delete").font(EMOJI_FONT))
                                    .on_press(Message::ConfirmDeleteGroupMessages(group_id.to_string()))
                                    .padding(10)
                                    .style(iced::theme::Button::Destructive),
                                button(text("❌ Cancel").font(EMOJI_FONT))
                                    .on_press(Message::CancelDelete)
                                    .padding(10),
                            ].spacing(20)
                        ].spacing(10).align_items(iced::Alignment::Center)
                    )
                    .padding(20)
                    .style(iced::theme::Container::Box)
                ].spacing(20).padding(20)
            } else {
                column![
                    back_button,
                    messages_area,
                    input_section,
                    chat_actions,
                ].spacing(20).padding(20)
            }
        } else {
            column![
                back_button,
                messages_area,
                input_section,
                chat_actions,
            ].spacing(20).padding(20)
        };
        
        container(content)
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }
    
    // Background listener per le notifiche dal server - SUPER OTTIMIZZATO
    async fn background_notification_listener(connection: Arc<Mutex<PersistentConnection>>) -> String {
        use tokio::time::{timeout, Duration};
        
        // Timeout drasticamente ridotto per maggiore reattività
        let mut conn = connection.lock().await;
        let mut notification = String::new();
        
        // Timeout ridotto da 1s a 500ms per reattività massima
        match timeout(Duration::from_millis(500), conn.reader.read_line(&mut notification)).await {
            Ok(Ok(0)) => {
                // Connessione chiusa
                "CONNECTION_CLOSED".to_string()
            }
            Ok(Ok(_)) => {
                let trimmed = notification.trim();
                if trimmed.starts_with("NOTIFICATION:") {
                    trimmed.to_string()
                } else if !trimmed.is_empty() {
                    // Messaggio non di notifica ricevuto, continua ad ascoltare
                    "CONTINUE_LISTENING".to_string()
                } else {
                    // Riga vuota, continua ad ascoltare
                    "CONTINUE_LISTENING".to_string()
                }
            }
            Ok(Err(_)) => {
                // Errore di lettura, riprova
                "CONTINUE_LISTENING".to_string()
            }
            Err(_) => {
                // Timeout, continua ad ascoltare senza delay aggiuntivo
                "CONTINUE_LISTENING".to_string()
            }
        }
    }
}

fn main() -> iced::Result {
    ChatApp::run(Settings::default())
}
