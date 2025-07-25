use eframe::egui;
use std::collections::{VecDeque, HashMap};
use std::time::Duration;
use uuid::Uuid;

mod network_manager;
use network_manager::{NetworkManager, NetworkMessage, NetworkCommand};

fn main() -> Result<(), eframe::Error> {
    env_logger::init();
    
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1200.0, 800.0])
            .with_min_inner_size([900.0, 650.0])
            .with_title("🦀 Ruggine Chat - Modern Client"),
        ..Default::default()
    };
    
    eframe::run_native(
        "🦀 Ruggine Chat",
        options,
        Box::new(|_cc| {
            Ok(Box::new(RuggineApp::new()))
        }),
    )
}

struct RuggineApp {
    // Network management
    network: NetworkManager,
    
    // Connection state
    server_host: String,
    server_port: String,
    is_connected: bool,
    connection_status: String,
    
    // User authentication
    username: String,
    is_registered: bool,
    registration_status: String,
    
    // Chat state
    messages: VecDeque<ChatMessage>,
    input_message: String,
    current_chat: CurrentChat,
    
    // Chat history per group/user
    chat_histories: HashMap<String, VecDeque<ChatMessage>>,
    
    // User interface state
    selected_tab: TabType,
    auto_scroll: bool,
    
    // Data collections
    online_users: Vec<String>,
    my_groups: Vec<GroupInfo>,
    pending_invites: Vec<InviteInfo>,
    
    // Input fields for operations
    new_group_name: String,
    invite_username: String,
    invite_group_name: String,
    private_message_target: String,
    
    // UI state
    show_connection_panel: bool,
    show_registration_panel: bool,
    show_create_group_dialog: bool,
    show_invite_dialog: bool,
    show_settings: bool,
    
    // Theme
    theme: Theme,
    
    // Error/success feedback
    last_feedback: Option<FeedbackMessage>,
}

#[derive(Clone, Debug)]
struct ChatMessage {
    id: String,
    content: String,
    sender: String,
    timestamp: String,
    message_type: ChatMessageType,
}

#[derive(Clone, Debug)]
enum ChatMessageType {
    UserMessage,
    SystemInfo,
    SystemError,
    GroupMessage,
    PrivateMessage,
}

#[derive(Clone, Debug)]
struct GroupInfo {
    name: String,
    member_count: usize,
    unread_count: usize,
}

#[derive(Clone, Debug)]
struct InviteInfo {
    id: String,
    from_user: String,
    group_name: String,
    timestamp: String,
}

#[derive(Clone, Debug, PartialEq)]
enum CurrentChat {
    None,
    Group(String),
    Private(String),
}

#[derive(Clone, Debug, PartialEq)]
enum TabType {
    Chat,
    Groups,
    Users,
    Invites,
    Settings,
}

#[derive(Clone)]
struct FeedbackMessage {
    text: String,
    is_error: bool,
    timestamp: std::time::Instant,
}

#[derive(Clone)]
struct Theme {
    primary_color: egui::Color32,
    secondary_color: egui::Color32,
    success_color: egui::Color32,
    error_color: egui::Color32,
    background_color: egui::Color32,
    panel_color: egui::Color32,
    text_color: egui::Color32,
    accent_color: egui::Color32,
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            primary_color: egui::Color32::from_rgb(70, 130, 200),
            secondary_color: egui::Color32::from_rgb(100, 160, 220),
            success_color: egui::Color32::from_rgb(60, 180, 120),
            error_color: egui::Color32::from_rgb(220, 80, 80),
            background_color: egui::Color32::from_rgb(240, 242, 245),
            panel_color: egui::Color32::from_rgb(255, 255, 255),
            text_color: egui::Color32::from_rgb(50, 50, 50),
            accent_color: egui::Color32::from_rgb(255, 165, 0),
        }
    }
}

impl RuggineApp {
    fn new() -> Self {
        Self {
            network: NetworkManager::new(),
            server_host: "127.0.0.1".to_string(),
            server_port: "5000".to_string(),
            is_connected: false,
            connection_status: "Disconnected".to_string(),
            username: String::new(),
            is_registered: false,
            registration_status: String::new(),
            messages: VecDeque::new(),
            input_message: String::new(),
            current_chat: CurrentChat::None,
            chat_histories: HashMap::new(),
            selected_tab: TabType::Chat,
            auto_scroll: true,
            online_users: Vec::new(),
            my_groups: Vec::new(),
            pending_invites: Vec::new(),
            new_group_name: String::new(),
            invite_username: String::new(),
            invite_group_name: String::new(),
            private_message_target: String::new(),
            show_connection_panel: true,
            show_registration_panel: false,
            show_create_group_dialog: false,
            show_invite_dialog: false,
            show_settings: false,
            theme: Theme::default(),
            last_feedback: None,
        }
    }
    
    fn add_message(&mut self, content: String, sender: String, msg_type: ChatMessageType) {
        let timestamp = chrono::Local::now().format("%H:%M:%S").to_string();
        let message = ChatMessage {
            id: Uuid::new_v4().to_string(),
            content,
            sender,
            timestamp,
            message_type: msg_type,
        };
        
        // Add to current chat
        self.messages.push_back(message.clone());
        
        // Add to appropriate chat history
        let chat_key = match &self.current_chat {
            CurrentChat::Group(name) => format!("group:{}", name),
            CurrentChat::Private(name) => format!("private:{}", name),
            CurrentChat::None => "system".to_string(),
        };
        
        self.chat_histories.entry(chat_key.clone()).or_insert_with(VecDeque::new).push_back(message);
        
        // Keep only last 500 messages per chat
        if let Some(history) = self.chat_histories.get_mut(&chat_key) {
            while history.len() > 500 {
                history.pop_front();
            }
        }
        
        // Global messages limit
        while self.messages.len() > 500 {
            self.messages.pop_front();
        }
    }
    
    fn set_feedback(&mut self, text: String, is_error: bool) {
        self.last_feedback = Some(FeedbackMessage {
            text,
            is_error,
            timestamp: std::time::Instant::now(),
        });
    }
    
    fn send_command(&self, command: &str) {
        self.network.send_command(NetworkCommand::SendMessage(command.to_string()));
    }
    
    fn connect_to_server(&mut self) {
        let port = self.server_port.parse::<u16>().unwrap_or(5000);
        self.network.send_command(NetworkCommand::Connect(self.server_host.clone(), port));
        self.connection_status = "Connecting...".to_string();
    }
    
    fn disconnect_from_server(&mut self) {
        self.network.send_command(NetworkCommand::Disconnect);
        self.is_connected = false;
        self.is_registered = false;
        self.connection_status = "Disconnected".to_string();
        self.show_connection_panel = true;
        self.show_registration_panel = false;
    }
    
    fn register_user(&mut self) {
        if !self.username.trim().is_empty() {
            let command = format!("/register {}", self.username.trim());
            self.send_command(&command);
            self.registration_status = "Registering...".to_string();
        }
    }
    
    fn create_group(&mut self) {
        if !self.new_group_name.trim().is_empty() {
            let command = format!("/create_group {}", self.new_group_name.trim());
            self.send_command(&command);
            self.new_group_name.clear();
            self.show_create_group_dialog = false;
        }
    }
    
    fn invite_user_to_group(&mut self) {
        if !self.invite_username.trim().is_empty() && !self.invite_group_name.trim().is_empty() {
            let command = format!("/invite {} {}", self.invite_username.trim(), self.invite_group_name.trim());
            self.send_command(&command);
            self.invite_username.clear();
            self.invite_group_name.clear();
            self.show_invite_dialog = false;
        }
    }
    
    fn send_message(&mut self) {
        if self.input_message.trim().is_empty() {
            return;
        }
        
        let message = self.input_message.trim().to_string();
        
        match &self.current_chat {
            CurrentChat::Group(group_name) => {
                let command = format!("/send {} {}", group_name, message);
                self.send_command(&command);
            }
            CurrentChat::Private(username) => {
                let command = format!("/send_private {} {}", username, message);
                self.send_command(&command);
            }
            CurrentChat::None => {
                self.set_feedback("Please select a group or user to chat with".to_string(), true);
                return;
            }
        }
        
        // Add message to chat immediately for better UX
        self.add_message(
            message,
            self.username.clone(),
            match &self.current_chat {
                CurrentChat::Group(_) => ChatMessageType::GroupMessage,
                CurrentChat::Private(_) => ChatMessageType::PrivateMessage,
                CurrentChat::None => ChatMessageType::UserMessage,
            }
        );
        
        self.input_message.clear();
    }
    
    fn switch_to_group(&mut self, group_name: String) {
        self.current_chat = CurrentChat::Group(group_name.clone());
        self.load_chat_history(&format!("group:{}", group_name));
    }
    
    fn switch_to_private(&mut self, username: String) {
        self.current_chat = CurrentChat::Private(username.clone());
        self.load_chat_history(&format!("private:{}", username));
    }
    
    fn load_chat_history(&mut self, chat_key: &str) {
        if let Some(history) = self.chat_histories.get(chat_key) {
            self.messages = history.clone();
        } else {
            self.messages.clear();
        }
    }
    
    fn process_network_messages(&mut self) {
        let messages = self.network.get_messages();
        for message in messages {
            match message {
                NetworkMessage::Connected => {
                    self.is_connected = true;
                    self.connection_status = "Connected".to_string();
                    self.show_connection_panel = false;
                    self.show_registration_panel = true;
                    self.set_feedback("Connected to server!".to_string(), false);
                }
                NetworkMessage::Disconnected => {
                    self.is_connected = false;
                    self.is_registered = false;
                    self.connection_status = "Disconnected".to_string();
                    self.show_connection_panel = true;
                    self.show_registration_panel = false;
                    self.set_feedback("Disconnected from server".to_string(), true);
                }
                NetworkMessage::ServerResponse(response) => {
                    self.parse_server_response(&response);
                }
                NetworkMessage::Error(error) => {
                    self.set_feedback(format!("Network error: {}", error), true);
                }
            }
        }
    }
    
    fn parse_server_response(&mut self, response: &str) {
        if response.starts_with("OK:") {
            let message = response[3..].trim();
            
            if message.contains("Registered as:") {
                self.is_registered = true;
                self.show_registration_panel = false;
                self.registration_status = "Registered!".to_string();
                self.set_feedback("Successfully registered!".to_string(), false);
                
                // Auto-fetch initial data
                self.send_command("/users");
                self.send_command("/my_groups");
                self.send_command("/my_invites");
            } else if message.contains("Online users:") {
                self.parse_users_list(message);
            } else if message.contains("Your groups:") {
                self.parse_groups_list(message);
            } else if message.contains("Group") && message.contains("created") {
                self.send_command("/my_groups"); // Refresh groups
                self.set_feedback("Group created successfully!".to_string(), false);
            } else if message.contains("sent to") {
                self.set_feedback("Message sent!".to_string(), false);
            } else if message.contains("Invitation sent") {
                self.set_feedback("Invitation sent!".to_string(), false);
            }
            
            self.add_message(message.to_string(), "System".to_string(), ChatMessageType::SystemInfo);
        } else if response.starts_with("ERROR:") {
            let error = response[6..].trim();
            self.set_feedback(format!("Error: {}", error), true);
            self.add_message(error.to_string(), "System".to_string(), ChatMessageType::SystemError);
        }
    }
    
    fn parse_users_list(&mut self, message: &str) {
        if let Some(users_part) = message.strip_prefix("Online users: ") {
            self.online_users = users_part.split(", ")
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty() && s != &self.username)
                .collect();
        }
    }
    
    fn parse_groups_list(&mut self, message: &str) {
        if let Some(groups_part) = message.strip_prefix("Your groups: ") {
            self.my_groups = groups_part.split(", ")
                .map(|s| GroupInfo {
                    name: s.trim().to_string(),
                    member_count: 0, // TODO: get actual member count
                    unread_count: 0,
                })
                .filter(|g| !g.name.is_empty())
                .collect();
        }
    }
}

impl eframe::App for RuggineApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Process network messages
        self.process_network_messages();
        
        // Remove expired feedback
        if let Some(ref feedback) = self.last_feedback {
            if feedback.timestamp.elapsed() > Duration::from_secs(5) {
                self.last_feedback = None;
            }
        }
        
        // Request regular repaints
        ctx.request_repaint_after(Duration::from_millis(100));
        
        // Show appropriate UI based on connection state
        if self.show_connection_panel {
            self.show_connection_ui(ctx);
            return;
        }
        
        if self.show_registration_panel {
            self.show_registration_ui(ctx);
            return;
        }
        
        // Main chat UI
        self.show_main_ui(ctx);
        
        // Dialogs
        if self.show_create_group_dialog {
            self.show_create_group_dialog_ui(ctx);
        }
        
        if self.show_invite_dialog {
            self.show_invite_dialog_ui(ctx);
        }
        
        if self.show_settings {
            self.show_settings_ui(ctx);
        }
    }
}

impl RuggineApp {
    fn show_connection_ui(&mut self, ctx: &egui::Context) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(150.0);
                
                // Title with custom styling
                ui.style_mut().text_styles.get_mut(&egui::TextStyle::Heading).unwrap().size = 32.0;
                ui.colored_label(self.theme.primary_color, "🦀 Ruggine Chat");
                
                ui.add_space(10.0);
                ui.label("Modern Chat Client");
                ui.add_space(40.0);
                
                // Connection form in a styled frame
                egui::Frame::none()
                    .fill(self.theme.panel_color)
                    .rounding(10.0)
                    .inner_margin(20.0)
                    .show(ui, |ui| {
                        ui.vertical_centered(|ui| {
                            ui.label("Connect to Server");
                            ui.add_space(15.0);
                            
                            egui::Grid::new("connection_grid")
                                .num_columns(2)
                                .spacing([15.0, 10.0])
                                .show(ui, |ui| {
                                    ui.label("🌐 Host:");
                                    ui.text_edit_singleline(&mut self.server_host);
                                    ui.end_row();
                                    
                                    ui.label("🔌 Port:");
                                    ui.text_edit_singleline(&mut self.server_port);
                                    ui.end_row();
                                });
                            
                            ui.add_space(20.0);
                            
                            if ui.add_sized([200.0, 35.0], egui::Button::new("🚀 Connect")
                                .fill(self.theme.primary_color)).clicked() {
                                self.connect_to_server();
                            }
                            
                            ui.add_space(10.0);
                            ui.label(&self.connection_status);
                        });
                    });
            });
        });
    }
    
    fn show_registration_ui(&mut self, ctx: &egui::Context) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(200.0);
                
                ui.heading("Welcome to Ruggine Chat!");
                ui.add_space(10.0);
                ui.label("Please choose a username to continue");
                ui.add_space(30.0);
                
                egui::Frame::none()
                    .fill(self.theme.panel_color)
                    .rounding(10.0)
                    .inner_margin(20.0)
                    .show(ui, |ui| {
                        ui.vertical_centered(|ui| {
                            ui.label("👤 Username");
                            ui.add_space(10.0);
                            
                            let response = ui.add_sized([250.0, 30.0], egui::TextEdit::singleline(&mut self.username)
                                .hint_text("Enter your username..."));
                            
                            ui.add_space(15.0);
                            
                            let register_btn = ui.add_sized([200.0, 35.0], egui::Button::new("📝 Register")
                                .fill(self.theme.success_color));
                            
                            if (register_btn.clicked() || (response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)))) 
                                && !self.username.trim().is_empty() {
                                self.register_user();
                            }
                            
                            ui.add_space(10.0);
                            ui.label(&self.registration_status);
                        });
                    });
            });
        });
    }
    
    fn show_main_ui(&mut self, ctx: &egui::Context) {
        // Top bar
        egui::TopBottomPanel::top("top_bar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 15.0;
                
                // Logo and title
                ui.colored_label(self.theme.primary_color, "🦀 Ruggine");
                ui.separator();
                
                // Tab buttons
                if ui.selectable_label(self.selected_tab == TabType::Chat, "💬 Chat").clicked() {
                    self.selected_tab = TabType::Chat;
                }
                if ui.selectable_label(self.selected_tab == TabType::Groups, "🏠 Groups").clicked() {
                    self.selected_tab = TabType::Groups;
                }
                if ui.selectable_label(self.selected_tab == TabType::Users, "👥 Users").clicked() {
                    self.selected_tab = TabType::Users;
                    self.send_command("/users");
                }
                if ui.selectable_label(self.selected_tab == TabType::Invites, "📨 Invites").clicked() {
                    self.selected_tab = TabType::Invites;
                    self.send_command("/my_invites");
                }
                
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    // Settings button
                    if ui.button("⚙️").clicked() {
                        self.show_settings = true;
                    }
                    
                    // Connection status
                    if self.is_connected {
                        ui.colored_label(self.theme.success_color, "🟢 Connected");
                    } else {
                        ui.colored_label(self.theme.error_color, "🔴 Disconnected");
                    }
                    
                    ui.separator();
                    
                    // Current chat info
                    match &self.current_chat {
                        CurrentChat::Group(name) => {
                            ui.label(format!("🏠 {}", name));
                        }
                        CurrentChat::Private(name) => {
                            ui.label(format!("💬 {}", name));
                        }
                        CurrentChat::None => {
                            ui.label("💭 No chat selected");
                        }
                    }
                    
                    ui.separator();
                    ui.label(format!("👤 {}", self.username));
                });
            });
        });
        
        // Show feedback message
        if let Some(ref feedback) = self.last_feedback {
            egui::TopBottomPanel::top("feedback").show(ctx, |ui| {
                ui.horizontal(|ui| {
                    let color = if feedback.is_error { 
                        self.theme.error_color 
                    } else { 
                        self.theme.success_color 
                    };
                    
                    let icon = if feedback.is_error { "❌" } else { "✅" };
                    ui.colored_label(color, format!("{} {}", icon, feedback.text));
                });
            });
        }
        
        // Main content area
        egui::CentralPanel::default().show(ctx, |ui| {
            match self.selected_tab {
                TabType::Chat => self.show_chat_tab(ui),
                TabType::Groups => self.show_groups_tab(ui),
                TabType::Users => self.show_users_tab(ui),
                TabType::Invites => self.show_invites_tab(ui),
                TabType::Settings => self.show_settings_tab(ui),
            }
        });
    }
    
    fn show_chat_tab(&mut self, ui: &mut egui::Ui) {
        ui.vertical(|ui| {
            // Chat header
            ui.horizontal(|ui| {
                match &self.current_chat {
                    CurrentChat::Group(name) => {
                        ui.label(format!("🏠 Group: {}", name));
                        if ui.button("🚪 Leave").clicked() {
                            let command = format!("/leave_group {}", name);
                            self.send_command(&command);
                        }
                    }
                    CurrentChat::Private(name) => {
                        ui.label(format!("💬 Private chat with: {}", name));
                    }
                    CurrentChat::None => {
                        ui.label("💭 Select a group or user to start chatting");
                    }
                }
                
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("🔄").clicked() {
                        self.send_command("/my_groups");
                        self.send_command("/users");
                    }
                });
            });
            
            ui.separator();
            
            // Messages area
            egui::ScrollArea::vertical()
                .auto_shrink([false; 2])
                .stick_to_bottom(self.auto_scroll)
                .show(ui, |ui| {
                    for message in &self.messages {
                        self.show_message(ui, message);
                    }
                });
            
            ui.separator();
            
            // Message input area
            ui.horizontal(|ui| {
                let input_response = ui.add_sized(
                    [ui.available_width() - 80.0, 30.0],
                    egui::TextEdit::singleline(&mut self.input_message)
                        .hint_text("Type your message...")
                );
                
                if ui.add_sized([70.0, 30.0], egui::Button::new("📤 Send")
                    .fill(self.theme.primary_color)).clicked() 
                    || (input_response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter))) {
                    self.send_message();
                }
            });
        });
    }
    
    fn show_groups_tab(&mut self, ui: &mut egui::Ui) {
        ui.vertical(|ui| {
            ui.horizontal(|ui| {
                ui.heading("🏠 Your Groups");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("➕ Create Group").clicked() {
                        self.show_create_group_dialog = true;
                    }
                    if ui.button("👋 Invite User").clicked() {
                        self.show_invite_dialog = true;
                    }
                    if ui.button("🔄 Refresh").clicked() {
                        self.send_command("/my_groups");
                    }
                });
            });
            
            ui.separator();
            
            if self.my_groups.is_empty() {
                ui.vertical_centered(|ui| {
                    ui.add_space(50.0);
                    ui.label("No groups yet");
                    ui.label("Create a group or wait for an invitation!");
                });
            } else {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    for group in &self.my_groups.clone() {
                        ui.group(|ui| {
                            ui.horizontal(|ui| {
                                ui.label(format!("🏠 {}", group.name));
                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    if ui.button("💬 Chat").clicked() {
                                        self.switch_to_group(group.name.clone());
                                        self.selected_tab = TabType::Chat;
                                    }
                                    if ui.button("🚪 Leave").clicked() {
                                        let command = format!("/leave_group {}", group.name);
                                        self.send_command(&command);
                                    }
                                });
                            });
                        });
                        ui.add_space(5.0);
                    }
                });
            }
        });
    }
    
    fn show_users_tab(&mut self, ui: &mut egui::Ui) {
        ui.vertical(|ui| {
            ui.horizontal(|ui| {
                ui.heading("👥 Online Users");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("🔄 Refresh").clicked() {
                        self.send_command("/users");
                    }
                });
            });
            
            ui.separator();
            
            if self.online_users.is_empty() {
                ui.vertical_centered(|ui| {
                    ui.add_space(50.0);
                    ui.label("No other users online");
                });
            } else {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    for user in &self.online_users.clone() {
                        ui.group(|ui| {
                            ui.horizontal(|ui| {
                                ui.label(format!("👤 {}", user));
                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    if ui.button("💬 Chat").clicked() {
                                        self.switch_to_private(user.clone());
                                        self.selected_tab = TabType::Chat;
                                    }
                                });
                            });
                        });
                        ui.add_space(5.0);
                    }
                });
            }
        });
    }
    
    fn show_invites_tab(&mut self, ui: &mut egui::Ui) {
        ui.vertical(|ui| {
            ui.horizontal(|ui| {
                ui.heading("📨 Pending Invites");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("🔄 Refresh").clicked() {
                        self.send_command("/my_invites");
                    }
                });
            });
            
            ui.separator();
            
            if self.pending_invites.is_empty() {
                ui.vertical_centered(|ui| {
                    ui.add_space(50.0);
                    ui.label("No pending invites");
                });
            } else {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    for invite in &self.pending_invites.clone() {
                        ui.group(|ui| {
                            ui.vertical(|ui| {
                                ui.label(format!("From: 👤 {}", invite.from_user));
                                ui.label(format!("Group: 🏠 {}", invite.group_name));
                                ui.label(format!("Time: 🕐 {}", invite.timestamp));
                                
                                ui.horizontal(|ui| {
                                    if ui.button("✅ Accept").clicked() {
                                        let command = format!("/accept_invite {}", invite.id);
                                        self.send_command(&command);
                                    }
                                    if ui.button("❌ Reject").clicked() {
                                        let command = format!("/reject_invite {}", invite.id);
                                        self.send_command(&command);
                                    }
                                });
                            });
                        });
                        ui.add_space(10.0);
                    }
                });
            }
        });
    }
    
    fn show_settings_tab(&mut self, ui: &mut egui::Ui) {
        ui.heading("⚙️ Settings");
        ui.separator();
        
        ui.checkbox(&mut self.auto_scroll, "📜 Auto-scroll chat");
        
        if ui.button("🔌 Disconnect").clicked() {
            self.disconnect_from_server();
        }
    }
    
    fn show_message(&self, ui: &mut egui::Ui, message: &ChatMessage) {
        ui.horizontal(|ui| {
            ui.label(format!("[{}]", message.timestamp));
            
            match &message.message_type {
                ChatMessageType::UserMessage => {
                    ui.colored_label(self.theme.primary_color, format!("👤 {}:", message.sender));
                }
                ChatMessageType::SystemInfo => {
                    ui.colored_label(self.theme.success_color, "ℹ️ System:");
                }
                ChatMessageType::SystemError => {
                    ui.colored_label(self.theme.error_color, "❌ Error:");
                }
                ChatMessageType::GroupMessage => {
                    ui.colored_label(self.theme.accent_color, format!("🏠 {}:", message.sender));
                }
                ChatMessageType::PrivateMessage => {
                    ui.colored_label(self.theme.secondary_color, format!("💬 {}:", message.sender));
                }
            }
            
            ui.label(&message.content);
        });
    }
    
    fn show_create_group_dialog_ui(&mut self, ctx: &egui::Context) {
        egui::Window::new("➕ Create New Group")
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .resizable(false)
            .show(ctx, |ui| {
                ui.vertical(|ui| {
                    ui.label("Enter group name:");
                    ui.text_edit_singleline(&mut self.new_group_name);
                    
                    ui.add_space(10.0);
                    
                    ui.horizontal(|ui| {
                        if ui.button("✅ Create").clicked() {
                            self.create_group();
                        }
                        if ui.button("❌ Cancel").clicked() {
                            self.show_create_group_dialog = false;
                            self.new_group_name.clear();
                        }
                    });
                });
            });
    }
    
    fn show_invite_dialog_ui(&mut self, ctx: &egui::Context) {
        egui::Window::new("👋 Invite User to Group")
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .resizable(false)
            .show(ctx, |ui| {
                ui.vertical(|ui| {
                    ui.label("Username:");
                    ui.text_edit_singleline(&mut self.invite_username);
                    
                    ui.label("Group name:");
                    ui.text_edit_singleline(&mut self.invite_group_name);
                    
                    ui.add_space(10.0);
                    
                    ui.horizontal(|ui| {
                        if ui.button("📤 Send Invite").clicked() {
                            self.invite_user_to_group();
                        }
                        if ui.button("❌ Cancel").clicked() {
                            self.show_invite_dialog = false;
                            self.invite_username.clear();
                            self.invite_group_name.clear();
                        }
                    });
                });
            });
    }
    
    fn show_settings_ui(&mut self, ctx: &egui::Context) {
        egui::Window::new("⚙️ Settings")
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.vertical(|ui| {
                    ui.heading("App Settings");
                    ui.separator();
                    
                    ui.checkbox(&mut self.auto_scroll, "📜 Auto-scroll messages");
                    
                    ui.add_space(10.0);
                    
                    if ui.button("🔌 Disconnect from Server").clicked() {
                        self.disconnect_from_server();
                        self.show_settings = false;
                    }
                    
                    ui.add_space(10.0);
                    
                    if ui.button("❌ Close").clicked() {
                        self.show_settings = false;
                    }
                });
            });
    }
}
