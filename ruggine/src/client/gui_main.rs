use iced::{
    widget::{button, column, container, row, text, text_input, scrollable, radio},
    Application, Command, Element, Length, Settings, Theme, Font, Color,
};
use log::error;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::net::TcpStream;
use tokio::io::{AsyncWriteExt, AsyncBufReadExt, BufWriter, BufReader};

use ruggine::client::ClientConfig;

// Font per emoji
const EMOJI_FONT: Font = Font::with_name("Segoe UI Emoji");
// Font grassetto per i titoli
const BOLD_FONT: Font = Font {
    family: iced::font::Family::SansSerif,
    weight: iced::font::Weight::Bold,
    ..Font::DEFAULT
};

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
    LeaveSpecificGroup(String),
    AcceptSpecificInvite(String),
    InviteToSpecificGroup(String), // Nuovo: invita a un gruppo specifico
    ShowInviteFormForGroup(String), // Nuovo: mostra form invito per gruppo
    
    // Gestione lista utenti per inviti 
    ShowUsersListForGroup(String), // Mostra lista utenti per invitare al gruppo
    InviteUserToGroup(String, String), // (username, group_name)
    HideUsersListForGroup, // Nascondi lista utenti
    
    // Gestione form di invito per gruppo specifico
    InviteToGroupUsernameChanged(String),
    SendInviteToGroup, // Non ha bisogno del parametro nel match, usa invite_form_group invece
    CancelInviteToGroup,
    RejectSpecificInvite(String),
    
    // Aggiornamento delle liste
    GroupsListUpdated(Vec<String>),
    InvitesListUpdated(Vec<(String, String)>),
    UsersListUpdated(Vec<String>), // Lista utenti per inviti
    
    // Dummy message per update interno
    None,
}

#[derive(Debug, Clone)]
pub enum AppState {
    Registration,  // Schermata iniziale per connessione e registrazione
    MainActions,   // Schermata principale con tutte le azioni
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
    
    // Input fields
    username: String,
    group_name: String,
    invite_username: String,  // Username da invitare
    invite_group: String,     // Gruppo per l'invito
    invite_id: String,        // ID dell'invito da accettare/rifiutare
    leave_group_name: String, // Nome del gruppo da abbandonare
    
    // UI state
    messages: Vec<String>,
    
    // Stati delle sezioni espanse
    groups_expanded: bool,
    invites_expanded: bool,
    
    // Stati delle sottosezioni
    groups_list_view: bool,
    groups_create_view: bool,
    
    // Dati delle liste
    my_groups: Vec<String>,
    my_invites: Vec<(String, String)>, // (invite_id, group_name)
    available_users: Vec<String>, // Lista utenti disponibili per inviti
    
    // Gestione form di invito per gruppo specifico
    invite_form_group: Option<String>, // Gruppo per cui è aperto il form di invito
    invite_to_group_username: String,  // Username da invitare nel form specifico
    users_list_for_group: Option<String>, // Gruppo per cui è aperta la lista utenti
    
    // Configurazione
    config: Option<ClientConfig>,
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
        
        (
            Self {
                app_state: AppState::Registration,
                connection_state: ConnectionState::Disconnected,
                connection_mode: ConnectionMode::Localhost,
                manual_host: String::new(),
                manual_port: String::new(),
                persistent_connection: None,
                username: String::new(),
                group_name: String::new(),
                invite_username: String::new(),
                invite_group: String::new(),
                invite_id: String::new(),
                leave_group_name: String::new(),
                messages: Vec::new(),
                groups_expanded: false,
                invites_expanded: false,
                groups_list_view: false,
                groups_create_view: false,
                my_groups: Vec::new(),
                my_invites: Vec::new(),
                available_users: Vec::new(),
                invite_form_group: None,
                invite_to_group_username: String::new(),
                users_list_for_group: None,
                config,
            },
            Command::none(),
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
                        self.messages.push("Connected to server!".to_string());
                        Command::none()
                    }
                    Err(error) => {
                        self.connection_state = ConnectionState::Error(error.clone());
                        self.messages.push(format!("Connection failed: {}", error));
                        Command::none()
                    }
                }
            }
            
            Message::StartMessageListener => {
                // Avvia il listener per i messaggi del server
                Command::none()
            }
            
            Message::ServerMessage(msg) => {
                if !msg.trim().is_empty() {
                    self.messages.push(format!("SERVER: {}", msg.trim()));
                    
                    // Auto-refresh delle liste quando necessario
                    if msg.contains("[Leave]") && self.groups_list_view {
                        // Ricarica la lista dei gruppi dopo aver lasciato un gruppo
                        if let Some(connection) = &self.persistent_connection {
                            let conn = connection.clone();
                            return Command::perform(
                                Self::send_command_persistent(conn, "/my_groups".to_string()),
                                |result| match result {
                                    Ok(response) => {
                                        let groups = Self::parse_groups_response(&response);
                                        Message::GroupsListUpdated(groups)
                                    }
                                    Err(e) => Message::ServerMessage(format!("Error refreshing groups: {}", e))
                                }
                            );
                        }
                    } else if (msg.contains("[Accept]") || msg.contains("[Reject]")) && self.invites_expanded {
                        // Ricarica la lista degli inviti dopo aver accettato/rifiutato
                        if let Some(connection) = &self.persistent_connection {
                            let conn = connection.clone();
                            return Command::perform(
                                Self::send_command_persistent(conn, "/my_invites".to_string()),
                                |result| match result {
                                    Ok(response) => {
                                        let invites = Self::parse_invites_response(&response);
                                        Message::InvitesListUpdated(invites)
                                    }
                                    Err(e) => Message::ServerMessage(format!("Error refreshing invites: {}", e))
                                }
                            );
                        }
                    }
                }
                Command::none()
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
                        self.messages.push(format!("Connecting and registering as '{}'...", username));
                        
                        Command::perform(
                            Self::connect_and_register_persistent(host, port, username),
                            |result| match result {
                                Ok((response, connection)) => {
                                    if response.contains("OK:") {
                                        Message::RegistrationSuccess(connection)
                                    } else {
                                        Message::ServerMessage(format!("Registration failed: {}", response))
                                    }
                                }
                                Err(e) => Message::ServerMessage(format!("Error: {}", e))
                            }
                        )
                    } else {
                        self.messages.push("Connection info not available".to_string());
                        Command::none()
                    }
                } else {
                    self.messages.push("WARNING: Username cannot be empty".to_string());
                    Command::none()
                }
            }
            
            Message::RegistrationSuccess(connection) => {
                self.connection_state = ConnectionState::Registered;
                self.app_state = AppState::MainActions;
                self.persistent_connection = Some(connection);
                self.messages.push("Registration successful! Welcome to Ruggine Chat.".to_string());
                Command::none()
            }
            
            Message::DisconnectPressed => {
                // Chiudi la connessione e torna alla schermata di registrazione
                self.persistent_connection = None;
                self.connection_state = ConnectionState::Disconnected;
                self.app_state = AppState::Registration;
                self.messages.clear();
                self.messages.push("Disconnected from server.".to_string());
                Command::none()
            }
            
            Message::ListUsersPressed => {
                if matches!(self.connection_state, ConnectionState::Registered) {
                    if let Some(connection) = &self.persistent_connection {
                        self.messages.push("Requesting user list...".to_string());
                        let conn = connection.clone();
                        Command::perform(
                            Self::send_command_persistent(conn, "/users".to_string()),
                            |result| match result {
                                Ok(response) => Message::ServerMessage(format!("[Users] {}", response)),
                                Err(e) => Message::ServerMessage(format!("Error: {}", e))
                            }
                        )
                    } else {
                        self.messages.push("No persistent connection available".to_string());
                        Command::none()
                    }
                } else {
                    self.messages.push("Please register first".to_string());
                    Command::none()
                }
            }
            
            Message::CreateGroupPressed => {
                if matches!(self.connection_state, ConnectionState::Registered) {
                    let group_name = self.group_name.clone();
                    if !group_name.is_empty() {
                        if let Some(connection) = &self.persistent_connection {
                            self.messages.push(format!("Creating group '{}'...", group_name));
                            let conn = connection.clone();
                            let command = format!("/create_group {}", group_name);
                            Command::perform(
                                Self::send_command_persistent(conn, command),
                                |result| match result {
                                    Ok(response) => Message::ServerMessage(format!("[Group] {}", response)),
                                    Err(e) => Message::ServerMessage(format!("Error: {}", e))
                                }
                            )
                        } else {
                            self.messages.push("No persistent connection available".to_string());
                            Command::none()
                        }
                    } else {
                        self.messages.push("WARNING: Group name cannot be empty".to_string());
                        Command::none()
                    }
                } else {
                    self.messages.push("Please register first".to_string());
                    Command::none()
                }
            }
            
            Message::ListGroupsPressed => {
                if matches!(self.connection_state, ConnectionState::Registered) {
                    if let Some(connection) = &self.persistent_connection {
                        self.messages.push("Requesting my groups...".to_string());
                        let conn = connection.clone();
                        Command::perform(
                            Self::send_command_persistent(conn, "/my_groups".to_string()),
                            |result| match result {
                                Ok(response) => {
                                    // Parse la risposta per estrarre i nomi dei gruppi
                                    let groups = Self::parse_groups_response(&response);
                                    Message::GroupsListUpdated(groups)
                                }
                                Err(e) => Message::ServerMessage(format!("Error: {}", e))
                            }
                        )
                    } else {
                        self.messages.push("No persistent connection available".to_string());
                        Command::none()
                    }
                } else {
                    self.messages.push("Please register first".to_string());
                    Command::none()
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
                            self.messages.push(format!("Inviting '{}' to group '{}'...", username, group_name));
                            let conn = connection.clone();
                            let command = format!("/invite {} {}", username, group_name);
                            Command::perform(
                                Self::send_command_persistent(conn, command),
                                |result| match result {
                                    Ok(response) => Message::ServerMessage(format!("[Invite] {}", response)),
                                    Err(e) => Message::ServerMessage(format!("Error: {}", e))
                                }
                            )
                        } else {
                            self.messages.push("No persistent connection available".to_string());
                            Command::none()
                        }
                    } else {
                        self.messages.push("WARNING: Username and group name cannot be empty".to_string());
                        Command::none()
                    }
                } else {
                    self.messages.push("Please register first".to_string());
                    Command::none()
                }
            }
            
            Message::ListInvitesPressed => {
                if matches!(self.connection_state, ConnectionState::Registered) {
                    if let Some(connection) = &self.persistent_connection {
                        self.messages.push("Requesting pending invites...".to_string());
                        let conn = connection.clone();
                        Command::perform(
                            Self::send_command_persistent(conn, "/my_invites".to_string()),
                            |result| match result {
                                Ok(response) => {
                                    // Parse la risposta per estrarre gli inviti
                                    let invites = Self::parse_invites_response(&response);
                                    Message::InvitesListUpdated(invites)
                                }
                                Err(e) => Message::ServerMessage(format!("Error: {}", e))
                            }
                        )
                    } else {
                        self.messages.push("No persistent connection available".to_string());
                        Command::none()
                    }
                } else {
                    self.messages.push("Please register first".to_string());
                    Command::none()
                }
            }
            
            Message::AcceptInvitePressed => {
                if matches!(self.connection_state, ConnectionState::Registered) {
                    let invite_id = self.invite_id.clone();
                    
                    if !invite_id.is_empty() {
                        if let Some(connection) = &self.persistent_connection {
                            self.messages.push(format!("Accepting invite '{}'...", invite_id));
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
                            self.messages.push("No persistent connection available".to_string());
                            Command::none()
                        }
                    } else {
                        self.messages.push("WARNING: Invite ID cannot be empty".to_string());
                        Command::none()
                    }
                } else {
                    self.messages.push("Please register first".to_string());
                    Command::none()
                }
            }
            
            Message::RejectInvitePressed => {
                if matches!(self.connection_state, ConnectionState::Registered) {
                    let invite_id = self.invite_id.clone();
                    
                    if !invite_id.is_empty() {
                        if let Some(connection) = &self.persistent_connection {
                            self.messages.push(format!("Rejecting invite '{}'...", invite_id));
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
                            self.messages.push("No persistent connection available".to_string());
                            Command::none()
                        }
                    } else {
                        self.messages.push("WARNING: Invite ID cannot be empty".to_string());
                        Command::none()
                    }
                } else {
                    self.messages.push("Please register first".to_string());
                    Command::none()
                }
            }
            
            Message::LeaveGroupPressed => {
                if matches!(self.connection_state, ConnectionState::Registered) {
                    let group_name = self.leave_group_name.clone();
                    
                    if !group_name.is_empty() {
                        if let Some(connection) = &self.persistent_connection {
                            self.messages.push(format!("Leaving group '{}'...", group_name));
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
                            self.messages.push("No persistent connection available".to_string());
                            Command::none()
                        }
                    } else {
                        self.messages.push("WARNING: Group name cannot be empty".to_string());
                        Command::none()
                    }
                } else {
                    self.messages.push("Please register first".to_string());
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
                // Quando si apre la sezione inviti, carica automaticamente la lista
                if self.invites_expanded {
                    if matches!(self.connection_state, ConnectionState::Registered) {
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
                self.groups_list_view = true;
                self.groups_create_view = false;
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
                    }
                }
                Command::none()
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
            
            Message::LeaveSpecificGroup(group_name) => {
                if matches!(self.connection_state, ConnectionState::Registered) {
                    if let Some(connection) = &self.persistent_connection {
                        self.messages.push(format!("Leaving group '{}'...", group_name));
                        let conn = connection.clone();
                        let command = format!("/leave_group {}", group_name);
                        Command::perform(
                            Self::send_command_persistent(conn, command),
                            |result| match result {
                                Ok(response) => {
                                    // Dopo aver lasciato il gruppo, ricarica la lista
                                    Message::ServerMessage(format!("[Leave] {} - Refreshing list...", response))
                                }
                                Err(e) => Message::ServerMessage(format!("Error: {}", e))
                            }
                        )
                    } else {
                        self.messages.push("No persistent connection available".to_string());
                        Command::none()
                    }
                } else {
                    self.messages.push("Please register first".to_string());
                    Command::none()
                }
            }
            
            Message::AcceptSpecificInvite(invite_id) => {
                if matches!(self.connection_state, ConnectionState::Registered) {
                    if let Some(connection) = &self.persistent_connection {
                        self.messages.push(format!("Accepting invite '{}'...", invite_id));
                        let conn = connection.clone();
                        let command = format!("/accept_invite {}", invite_id);
                        Command::perform(
                            Self::send_command_persistent(conn, command),
                            |result| match result {
                                Ok(response) => {
                                    Message::ServerMessage(format!("[Accept] {}", response))
                                }
                                Err(e) => Message::ServerMessage(format!("Error: {}", e))
                            }
                        )
                    } else {
                        self.messages.push("No persistent connection available".to_string());
                        Command::none()
                    }
                } else {
                    self.messages.push("Please register first".to_string());
                    Command::none()
                }
            }
            
            Message::RejectSpecificInvite(invite_id) => {
                if matches!(self.connection_state, ConnectionState::Registered) {
                    if let Some(connection) = &self.persistent_connection {
                        self.messages.push(format!("Rejecting invite '{}'...", invite_id));
                        let conn = connection.clone();
                        let command = format!("/reject_invite {}", invite_id);
                        Command::perform(
                            Self::send_command_persistent(conn, command),
                            |result| match result {
                                Ok(response) => {
                                    Message::ServerMessage(format!("[Reject] {}", response))
                                }
                                Err(e) => Message::ServerMessage(format!("Error: {}", e))
                            }
                        )
                    } else {
                        self.messages.push("No persistent connection available".to_string());
                        Command::none()
                    }
                } else {
                    self.messages.push("Please register first".to_string());
                    Command::none()
                }
            }
            
            Message::GroupsListUpdated(groups) => {
                self.my_groups = groups;
                self.messages.push(format!("Groups list updated: {} groups found", self.my_groups.len()));
                Command::none()
            }
            
            Message::InvitesListUpdated(invites) => {
                self.my_invites = invites;
                self.messages.push(format!("Invites list updated: {} invites found", self.my_invites.len()));
                Command::none()
            }
            
            Message::UsersListUpdated(users) => {
                self.available_users = users;
                self.messages.push(format!("Users list updated: {} users found", self.available_users.len()));
                Command::none()
            }
            
            // Gestione lista utenti per inviti a gruppi
            Message::ShowUsersListForGroup(group_name) => {
                // Mostra la lista degli utenti per invitare al gruppo
                self.users_list_for_group = Some(group_name);
                self.invite_form_group = None; // Chiudi il form se era aperto
                
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
                        self.messages.push("No persistent connection available".to_string());
                        Command::none()
                    }
                } else {
                    self.messages.push("Please register first".to_string());
                    Command::none()
                }
            }
            
            Message::InviteUserToGroup(username, group_name) => {
                // Invia l'invito direttamente
                if matches!(self.connection_state, ConnectionState::Registered) {
                    if let Some(connection) = &self.persistent_connection {
                        self.messages.push(format!("Inviting '{}' to group '{}'...", username, group_name));
                        let conn = connection.clone();
                        let command = format!("/invite {} {}", username, group_name);
                        
                        Command::perform(
                            Self::send_command_persistent(conn, command),
                            |result| match result {
                                Ok(response) => Message::ServerMessage(format!("[Invite] {}", response)),
                                Err(e) => Message::ServerMessage(format!("Error: {}", e))
                            }
                        )
                    } else {
                        self.messages.push("No persistent connection available".to_string());
                        Command::none()
                    }
                } else {
                    self.messages.push("Please register first".to_string());
                    Command::none()
                }
            }
            
            Message::HideUsersListForGroup => {
                // Nascondi la lista degli utenti
                self.users_list_for_group = None;
                Command::none()
            }
            
            // Nuovi casi per la gestione degli inviti per gruppo specifico
            Message::InviteToSpecificGroup(group_name) => {
                // Mostra il form di invito per il gruppo specifico
                self.invite_form_group = Some(group_name);
                self.invite_to_group_username.clear();
                Command::none()
            }
            
            Message::ShowInviteFormForGroup(group_name) => {
                // Mostra direttamente la lista degli utenti invece del form
                self.users_list_for_group = Some(group_name);
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
                        self.messages.push("No persistent connection available".to_string());
                        Command::none()
                    }
                } else {
                    self.messages.push("Please register first".to_string());
                    Command::none()
                }
            }
            
            Message::InviteToGroupUsernameChanged(username) => {
                // Aggiorna il campo username per l'invito al gruppo
                self.invite_to_group_username = username;
                Command::none()
            }
            
            Message::SendInviteToGroup => {
                // Invia l'invito al gruppo specifico
                if matches!(self.connection_state, ConnectionState::Registered) {
                    if let Some(group_name) = &self.invite_form_group {
                        let username = self.invite_to_group_username.clone();
                        
                        if !username.is_empty() {
                            if let Some(connection) = &self.persistent_connection {
                                self.messages.push(format!("Inviting '{}' to group '{}'...", username, group_name));
                                let conn = connection.clone();
                                let command = format!("/invite {} {}", username, group_name);
                                
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
                                self.messages.push("No persistent connection available".to_string());
                                Command::none()
                            }
                        } else {
                            self.messages.push("WARNING: Username cannot be empty".to_string());
                            Command::none()
                        }
                    } else {
                        self.messages.push("No group selected for invite".to_string());
                        Command::none()
                    }
                } else {
                    self.messages.push("Please register first".to_string());
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
            
            Message::None => Command::none(),
        }
    }

    fn view(&self) -> Element<Message> {
        match self.app_state {
            AppState::Registration => self.view_registration(),
            AppState::MainActions => self.view_main_actions(),
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

        let messages_list = if !self.messages.is_empty() {
            scrollable(
                column(
                    self.messages
                        .iter()
                        .map(|msg| text(msg).into())
                        .collect::<Vec<_>>()
                )
                .spacing(5)
                .padding(10)
            )
            .height(Length::Fixed(150.0))
        } else {
            scrollable(column![]).height(Length::Fixed(0.0))
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
            
            messages_list,
        ]
        .spacing(15)
        .padding(30)
        .align_items(iced::Alignment::Center);

        container(content)
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
        
        let users_section = column![
            users_header,
            button(text("👥 List Users").font(EMOJI_FONT))
                .on_press(Message::ListUsersPressed)
                .padding(5),
        ].spacing(5);

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
                                .map(|group| {
                                    row![
                                        text(format!("👥 {}", group)).width(Length::Fill).font(EMOJI_FONT),
                                        button(text("➕").font(EMOJI_FONT))
                                            .on_press(Message::ShowInviteFormForGroup(group.clone()))
                                            .padding(5)
                                            .width(Length::Fixed(35.0)),
                                        button(text("❌ Leave").font(EMOJI_FONT))
                                            .on_press(Message::LeaveSpecificGroup(group.clone()))
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

        // Sezione messaggi (sempre visibile)
        let messages_section = column![
            text("Messages").size(18),
            scrollable(
                column(
                    self.messages
                        .iter()
                        .map(|msg| text(msg).into())
                        .collect::<Vec<_>>()
                )
                .spacing(5)
                .padding(10)
            )
            .height(Length::Fixed(200.0)),
        ].spacing(5);

        let content = column![
            header,
            
            text(status_text).size(16),
            text(format!("Logged in as: {}", self.username)).size(14),
            
            users_section,
            groups_section,
            invites_section,
            messages_section,
        ]
        .spacing(15)
        .padding(20);

        container(content)
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
    
    async fn test_connection(host: String, port: String) -> Result<(), String> {
        let address = format!("{}:{}", host, port);
        match TcpStream::connect(&address).await {
            Ok(_) => Ok(()),
            Err(e) => Err(format!("Failed to connect to {}: {}", address, e))
        }
    }
    
    async fn connect_and_register_persistent(
        host: String, 
        port: String, 
        username: String
    ) -> Result<(String, Arc<Mutex<PersistentConnection>>), Box<dyn std::error::Error + Send + Sync>> {
        let address = format!("{}:{}", host, port);
        
        // Connetti al server
        let stream = TcpStream::connect(&address).await?;
        
        // Crea reader e writer (owned)
        let (reader, writer) = stream.into_split();
        let mut writer = BufWriter::new(writer);
        let mut reader = BufReader::new(reader);
        
        // Leggi il messaggio di benvenuto
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        let mut welcome_lines = Vec::new();
        loop {
            let mut line = String::new();
            match tokio::time::timeout(tokio::time::Duration::from_millis(50), reader.read_line(&mut line)).await {
                Ok(Ok(0)) => break,
                Ok(Ok(_)) => {
                    welcome_lines.push(line.trim().to_string());
                }
                _ => break,
            }
        }
        
        // Invia il comando di registrazione
        let command = format!("/register {}", username);
        writer.write_all(command.as_bytes()).await?;
        writer.write_all(b"\n").await?;
        writer.flush().await?;
        
        // Leggi la risposta
        let mut response = String::new();
        reader.read_line(&mut response).await?;
        
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
        let mut conn = connection.lock().await;
        
        // Invia il comando
        conn.writer.write_all(command.as_bytes()).await?;
        conn.writer.write_all(b"\n").await?;
        conn.writer.flush().await?;
        
        // Leggi la risposta (potrebbe essere multi-riga)
        let mut full_response = String::new();
        let mut first_line = String::new();
        conn.reader.read_line(&mut first_line).await?;
        full_response.push_str(&first_line);
        
        // Se la prima riga contiene "Your pending invites:" o altri pattern multi-riga,
        // continua a leggere le righe successive
        if first_line.contains("Your pending invites:") || 
           first_line.contains("Your groups:") ||
           first_line.contains("Online users:") ||
           first_line.contains("All users:") {
            
            // Leggi righe aggiuntive con timeout
            loop {
                let mut line = String::new();
                match tokio::time::timeout(tokio::time::Duration::from_millis(100), conn.reader.read_line(&mut line)).await {
                    Ok(Ok(0)) => break, // Connessione chiusa
                    Ok(Ok(_)) => {
                        if line.trim().is_empty() {
                            break; // Riga vuota indica fine della risposta multi-riga
                        }
                        full_response.push_str(&line);
                    }
                    _ => break, // Timeout o errore - fine della risposta
                }
            }
        }
        
        Ok(full_response.trim().to_string())
    }
    
    async fn connect_and_register(host: String, port: String, username: String) -> Result<String, Box<dyn std::error::Error>> {
        let command = format!("/register {}", username);
        let response = Self::send_tcp_command(&host, &port, &command).await?;
        Ok(response)
    }
    
    async fn send_tcp_command(host: &str, port: &str, command: &str) -> Result<String, Box<dyn std::error::Error>> {
        let address = format!("{}:{}", host, port);
        
        // Connetti al server
        let stream = TcpStream::connect(&address).await?;
        
        // Crea reader e writer
        let (reader, writer) = stream.into_split();
        let mut writer = BufWriter::new(writer);
        let mut reader = BufReader::new(reader);
        
        // Leggi il messaggio di benvenuto (più righe) fino a quando non diventa silenzioso
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        let mut welcome_lines = Vec::new();
        loop {
            let mut line = String::new();
            match tokio::time::timeout(tokio::time::Duration::from_millis(50), reader.read_line(&mut line)).await {
                Ok(Ok(0)) => break, // Connessione chiusa
                Ok(Ok(_)) => {
                    welcome_lines.push(line.trim().to_string());
                }
                _ => break, // Timeout o errore - non ci sono più dati
            }
        }
        
        // Invia il comando
        writer.write_all(command.as_bytes()).await?;
        writer.write_all(b"\n").await?;
        writer.flush().await?;
        
        // Leggi la risposta del comando
        let mut response = String::new();
        reader.read_line(&mut response).await?;
        
        Ok(response.trim().to_string())
    }
    
    async fn send_register_command(host: String, port: String, username: String) -> Result<String, Box<dyn std::error::Error>> {
        let command = format!("/register {}", username);
        let response = Self::send_tcp_command(&host, &port, &command).await?;
        Ok(format!("[Registration] {}", response))
    }
    
    async fn send_list_users_command(host: String, port: String) -> Result<String, Box<dyn std::error::Error>> {
        let response = Self::send_tcp_command(&host, &port, "/users").await?;
        Ok(format!("[Users] {}", response))
    }
    
    async fn send_create_group_command(host: String, port: String, group_name: String) -> Result<String, Box<dyn std::error::Error>> {
        let command = format!("/create_group {}", group_name);
        let response = Self::send_tcp_command(&host, &port, &command).await?;
        Ok(format!("[Group] {}", response))
    }
    
    async fn send_list_groups_command(host: String, port: String) -> Result<String, Box<dyn std::error::Error>> {
        let response = Self::send_tcp_command(&host, &port, "/my_groups").await?;
        Ok(format!("[My Groups] {}", response))
    }
    
    // Helper functions per parsare le risposte del server
    fn parse_groups_response(response: &str) -> Vec<String> {
        // Esempi di risposta: 
        // - "OK: Your groups: gruppo1, gruppo2, gruppo3"
        // - "OK: You are not in any groups"
        if response.starts_with("OK:") {
            if response.contains("You are not in any groups") {
                return Vec::new();
            } else if response.contains("Your groups:") {
                let groups_part = response.split("Your groups:").nth(1).unwrap_or("").trim();
                if groups_part.is_empty() {
                    Vec::new()
                } else {
                    groups_part
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
}

fn main() -> iced::Result {
    ChatApp::run(Settings::default())
}
