use eframe::egui;
use std::collections::VecDeque;
use std::time::Duration;

mod network_manager;
use network_manager::{NetworkManager, NetworkMessage, NetworkCommand};

fn main() -> Result<(), eframe::Error> {
    env_logger::init();
    
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1000.0, 750.0])
            .with_min_inner_size([800.0, 600.0])
            .with_title("Ruggine Chat - Modern GUI Client"),
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
    show_connection_dialog: bool,
    
    // User authentication
    username: String,
    is_registered: bool,
    
    // Chat state
    messages: VecDeque<ChatMessage>,
    input_message: String,
    current_group: String,
    
    // User interface state
    auto_scroll: bool,
    show_users_sidebar: bool,
    show_groups_sidebar: bool,
    show_help_dialog: bool,
    show_settings_dialog: bool,
    
    // Data collections
    online_users: Vec<String>,
    my_groups: Vec<String>,
    available_groups: Vec<String>,
    pending_invites: Vec<GroupInvite>,
    
    // Group management
    new_group_name: String,
    invite_username: String,
    invite_group_name: String,
    
    // UI theme
    dark_mode: bool,
    message_font_size: f32,
}

#[derive(Clone)]
struct ChatMessage {
    text: String,
    message_type: MessageType,
    timestamp: String,
}

#[derive(Clone)]
enum MessageType {
    UserMessage,
    SystemInfo,
    SystemError,
    ServerResponse,
    GroupMessage { from: String, group: String },
}

#[derive(Clone)]
struct GroupInvite {
    from_user: String,
    group_name: String,
    timestamp: String,
}

impl RuggineApp {
    fn new() -> Self {
        Self {
            network: NetworkManager::new(),
            server_host: "127.0.0.1".to_string(),
            server_port: "5000".to_string(),
            show_connection_dialog: true,
            username: String::new(),
            is_registered: false,
            messages: VecDeque::new(),
            input_message: String::new(),
            current_group: String::new(),
            auto_scroll: true,
            show_users_sidebar: false,
            show_groups_sidebar: false,
            show_help_dialog: false,
            show_settings_dialog: false,
            online_users: Vec::new(),
            my_groups: Vec::new(),
            available_groups: Vec::new(),
            pending_invites: Vec::new(),
            new_group_name: String::new(),
            invite_username: String::new(),
            invite_group_name: String::new(),
            dark_mode: true,
            message_font_size: 14.0,
        }
    }
    
    fn add_message(&mut self, text: String, msg_type: MessageType) {
        let timestamp = chrono::Local::now().format("%H:%M:%S").to_string();
        self.messages.push_back(ChatMessage {
            text,
            message_type: msg_type,
            timestamp,
        });
        
        // Keep only last 1000 messages for performance
        while self.messages.len() > 1000 {
            self.messages.pop_front();
        }
    }
    
    fn send_command(&self, command: &str) {
        self.network.send_command(NetworkCommand::SendMessage(command.to_string()));
    }
    
    fn connect_to_server(&self) {
        let port = self.server_port.parse::<u16>().unwrap_or(5000);
        self.network.send_command(NetworkCommand::Connect(self.server_host.clone(), port));
    }
    
    fn disconnect_from_server(&self) {
        self.network.send_command(NetworkCommand::Disconnect);
    }
    
    fn parse_server_response(&mut self, response: &str) {
        if response.starts_with("OK:") {
            let message = &response[3..].trim();
            self.add_message(format!("✅ {}", message), MessageType::SystemInfo);
            
            // Handle specific successful responses
            if message.contains("registered") {
                self.is_registered = true;
            } else if message.contains("Online users:") {
                if let Some(users_part) = message.split(':').nth(1) {
                    self.online_users = users_part
                        .split(',')
                        .map(|s| s.trim().to_string())
                        .filter(|s| !s.is_empty() && s != &self.username)
                        .collect();
                }
            } else if message.contains("Your groups:") {
                if let Some(groups_part) = message.split(':').nth(1) {
                    self.my_groups = groups_part
                        .split(',')
                        .map(|s| s.trim().to_string())
                        .filter(|s| !s.is_empty())
                        .collect();
                }
            } else if message.contains("Joined group") {
                if let Some(group_name) = message.split('\'').nth(1) {
                    self.current_group = group_name.to_string();
                }
            }
        } else if response.starts_with("ERROR:") {
            let error_msg = &response[6..].trim();
            self.add_message(format!("❌ {}", error_msg), MessageType::SystemError);
        } else if response.contains("[") && response.contains("]") {
            // Group message format: [GroupName] User: Message
            self.add_message(response.to_string(), MessageType::ServerResponse);
        } else {
            // General server response
            self.add_message(response.to_string(), MessageType::ServerResponse);
        }
    }
    
    fn send_message(&mut self) {
        if self.input_message.trim().is_empty() {
            return;
        }
        
        let message = self.input_message.trim().to_string();
        
        if message.starts_with('/') {
            // Command - first add to local display, then send
            self.add_message(format!("💻 Command: {}", message), MessageType::UserMessage);
            self.network.send_command(NetworkCommand::SendMessage(message));
        } else if !self.current_group.is_empty() {
            // Group message - prepare command and display message
            let current_group = self.current_group.clone(); // Clone to avoid borrow issues
            let full_command = format!("/msg {} {}", current_group, message);
            
            self.add_message(format!("💬 You to [{}]: {}", current_group, message), MessageType::UserMessage);
            self.network.send_command(NetworkCommand::SendMessage(full_command));
        } else {
            self.add_message("⚠️ Please join a group first or use a command (starting with /)".to_string(), MessageType::SystemError);
        }
        
        self.input_message.clear();
    }
}

impl eframe::App for RuggineApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Apply theme
        if self.dark_mode {
            ctx.set_visuals(egui::Visuals::dark());
        } else {
            ctx.set_visuals(egui::Visuals::light());
        }
        
        // Handle network messages
        let messages = self.network.get_messages();
        for msg in messages {
            match msg {
                NetworkMessage::Connected => {
                    self.show_connection_dialog = false;
                    self.add_message("🟢 Connected to Ruggine server!".to_string(), MessageType::SystemInfo);
                }
                NetworkMessage::Disconnected => {
                    self.show_connection_dialog = true;
                    self.is_registered = false;
                    self.current_group.clear();
                    self.add_message("🔴 Disconnected from server".to_string(), MessageType::SystemError);
                }
                NetworkMessage::ServerResponse(response) => {
                    self.parse_server_response(&response);
                }
                NetworkMessage::Error(error) => {
                    self.add_message(format!("💥 Network Error: {}", error), MessageType::SystemError);
                }
            }
        }
        
        // Request regular repaints for real-time updates
        ctx.request_repaint_after(Duration::from_millis(100));
        
        // Connection dialog
        if self.show_connection_dialog {
            self.show_connection_ui(ctx);
            return;
        }
        
        // Main UI
        self.show_main_ui(ctx);
        
        // Dialogs
        if self.show_help_dialog {
            self.show_help_ui(ctx);
        }
        
        if self.show_settings_dialog {
            self.show_settings_ui(ctx);
        }
    }
}

impl RuggineApp {
    fn show_connection_ui(&mut self, ctx: &egui::Context) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(100.0);
                
                ui.heading("🦀 Welcome to Ruggine Chat");
                ui.add_space(20.0);
                
                ui.label("Connect to your Ruggine server to start chatting");
                ui.add_space(30.0);
                
                egui::Grid::new("connection_grid")
                    .num_columns(2)
                    .spacing([10.0, 10.0])
                    .show(ui, |ui| {
                        ui.label("Server Address:");
                        ui.text_edit_singleline(&mut self.server_host);
                        ui.end_row();
                        
                        ui.label("Port:");
                        ui.text_edit_singleline(&mut self.server_port);
                        ui.end_row();
                    });
                
                ui.add_space(20.0);
                
                if ui.button("🔗 Connect to Server").clicked() {
                    self.connect_to_server();
                }
                
                ui.add_space(10.0);
                ui.small("Default: 127.0.0.1:5000");
                
                ui.add_space(50.0);
                
                if !self.network.is_connected() {
                    ui.horizontal(|ui| {
                        if ui.button("❓ Help").clicked() {
                            self.show_help_dialog = true;
                        }
                        if ui.button("⚙️ Settings").clicked() {
                            self.show_settings_dialog = true;
                        }
                    });
                }
            });
        });
    }
    
    fn show_main_ui(&mut self, ctx: &egui::Context) {
        // Top menu bar
        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                ui.menu_button("🔌 Connection", |ui| {
                    if ui.button("📊 Server Status").clicked() {
                        // Could show ping, connection info, etc.
                    }
                    ui.separator();
                    if ui.button("🔌 Disconnect").clicked() {
                        self.disconnect_from_server();
                    }
                });
                
                ui.menu_button("👤 Account", |ui| {
                    if !self.is_registered {
                        if ui.button("📝 Register").clicked() && !self.username.is_empty() {
                            self.send_command(&format!("/register {}", self.username));
                        }
                    }
                    if ui.button("👥 Show Online Users").clicked() {
                        self.send_command("/users");
                        self.show_users_sidebar = true;
                    }
                });
                
                ui.menu_button("🏠 Groups", |ui| {
                    if ui.button("📋 My Groups").clicked() {
                        self.send_command("/my_groups");
                        self.show_groups_sidebar = true;
                    }
                    if ui.button("➕ Create Group").clicked() && !self.new_group_name.is_empty() {
                        self.send_command(&format!("/create_group {}", self.new_group_name));
                        self.new_group_name.clear();
                    }
                });
                
                ui.menu_button("🔧 View", |ui| {
                    ui.checkbox(&mut self.show_users_sidebar, "👥 Users Panel");
                    ui.checkbox(&mut self.show_groups_sidebar, "🏠 Groups Panel");
                    ui.separator();
                    ui.checkbox(&mut self.auto_scroll, "📜 Auto Scroll");
                    if ui.button("🗑️ Clear Messages").clicked() {
                        self.messages.clear();
                    }
                });
                
                ui.menu_button("❓ Help", |ui| {
                    if ui.button("📚 Commands").clicked() {
                        self.show_help_dialog = true;
                    }
                    if ui.button("⚙️ Settings").clicked() {
                        self.show_settings_dialog = true;
                    }
                });
                
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    // Connection status
                    if self.network.is_connected() {
                        ui.colored_label(egui::Color32::GREEN, "🟢 Connected");
                    } else {
                        ui.colored_label(egui::Color32::RED, "🔴 Disconnected");
                    }
                    
                    // Current group
                    if !self.current_group.is_empty() {
                        ui.separator();
                        ui.label(format!("🏠 {}", self.current_group));
                    }
                    
                    // Username
                    if self.is_registered {
                        ui.separator();
                        ui.label(format!("👤 {}", self.username));
                    }
                });
            });
        });
        
        // Users sidebar
        if self.show_users_sidebar {
            egui::SidePanel::right("users_panel").default_width(220.0).show(ctx, |ui| {
                ui.heading("👥 Online Users");
                ui.separator();
                
                egui::ScrollArea::vertical().show(ui, |ui| {
                    if self.online_users.is_empty() {
                        ui.label("No other users online");
                    } else {
                        for user in &self.online_users.clone() {
                            ui.horizontal(|ui| {
                                ui.label(format!("👤 {}", user));
                                if ui.small_button("💌").on_hover_text("Invite to group").clicked() {
                                    self.invite_username = user.clone();
                                }
                            });
                        }
                    }
                });
                
                ui.separator();
                if ui.button("🔄 Refresh").clicked() {
                    self.send_command("/users");
                }
                if ui.button("❌ Close").clicked() {
                    self.show_users_sidebar = false;
                }
            });
        }
        
        // Groups sidebar
        if self.show_groups_sidebar {
            egui::SidePanel::left("groups_panel").default_width(280.0).show(ctx, |ui| {
                ui.heading("🏠 Group Management");
                ui.separator();
                
                // My Groups section
                ui.label("📋 My Groups:");
                egui::ScrollArea::vertical().max_height(150.0).show(ui, |ui| {
                    if self.my_groups.is_empty() {
                        ui.label("No groups yet");
                    } else {
                        for group in &self.my_groups.clone() {
                            ui.horizontal(|ui| {
                                ui.label(format!("🏠 {}", group));
                                if ui.small_button("🚪").on_hover_text("Join group").clicked() {
                                    self.send_command(&format!("/join_group {}", group));
                                }
                                if ui.small_button("🚶").on_hover_text("Leave group").clicked() {
                                    self.send_command(&format!("/leave_group {}", group));
                                    if self.current_group == *group {
                                        self.current_group.clear();
                                    }
                                }
                            });
                        }
                    }
                });
                
                ui.separator();
                
                // Create new group
                ui.label("➕ Create New Group:");
                ui.horizontal(|ui| {
                    ui.text_edit_singleline(&mut self.new_group_name);
                    if ui.button("Create").clicked() && !self.new_group_name.trim().is_empty() {
                        self.send_command(&format!("/create_group {}", self.new_group_name.trim()));
                        self.new_group_name.clear();
                    }
                });
                
                ui.separator();
                
                // Invite user to group
                ui.label("📧 Invite User:");
                egui::Grid::new("invite_grid").show(ui, |ui| {
                    ui.label("Username:");
                    ui.text_edit_singleline(&mut self.invite_username);
                    ui.end_row();
                    
                    ui.label("Group:");
                    ui.text_edit_singleline(&mut self.invite_group_name);
                    ui.end_row();
                });
                
                if ui.button("📤 Send Invite").clicked() 
                    && !self.invite_username.trim().is_empty() 
                    && !self.invite_group_name.trim().is_empty() {
                    self.send_command(&format!("/invite {} {}", 
                        self.invite_username.trim(), 
                        self.invite_group_name.trim()));
                    self.invite_username.clear();
                    self.invite_group_name.clear();
                }
                
                ui.separator();
                
                ui.horizontal(|ui| {
                    if ui.button("🔄 Refresh").clicked() {
                        self.send_command("/my_groups");
                    }
                    if ui.button("❌ Close").clicked() {
                        self.show_groups_sidebar = false;
                    }
                });
            });
        }
        
        // Bottom input panel
        egui::TopBottomPanel::bottom("input_panel").show(ctx, |ui| {
            if !self.is_registered {
                // Registration UI
                ui.horizontal(|ui| {
                    ui.label("👤 Choose username:");
                    ui.text_edit_singleline(&mut self.username);
                    if ui.button("📝 Register").clicked() && !self.username.trim().is_empty() {
                        self.send_command(&format!("/register {}", self.username.trim()));
                    }
                });
                ui.small("Register first to start using the chat");
            } else {
                // Message input UI
                ui.horizontal(|ui| {
                    ui.label("💬");
                    let response = ui.text_edit_singleline(&mut self.input_message);
                    
                    // Send on Enter
                    if response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                        self.send_message();
                    }
                    
                    if ui.button("📤 Send").clicked() {
                        self.send_message();
                    }
                    
                    ui.separator();
                    
                    // Quick actions
                    if ui.small_button("👥").on_hover_text("Show users").clicked() {
                        self.send_command("/users");
                        self.show_users_sidebar = true;
                    }
                    if ui.small_button("🏠").on_hover_text("Show groups").clicked() {
                        self.send_command("/my_groups");
                        self.show_groups_sidebar = true;
                    }
                });
                
                if !self.current_group.is_empty() {
                    ui.small(format!("📍 Sending to group: {}", self.current_group));
                } else {
                    ui.small("💡 Join a group to send messages, or use commands starting with /");
                }
            }
        });
        
        // Central message area
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("💬 Chat Messages");
            ui.separator();
            
            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .stick_to_bottom(self.auto_scroll)
                .show(ui, |ui| {
                    if self.messages.is_empty() {
                        ui.centered_and_justified(|ui| {
                            ui.label("Welcome to Ruggine Chat! 🦀\n\nRegister and join a group to start chatting.\nUse the sidebar panels to manage users and groups.");
                        });
                    } else {
                        for msg in &self.messages {
                            ui.horizontal(|ui| {
                                ui.label(format!("[{}]", msg.timestamp));
                                
                                let (color, icon) = match &msg.message_type {
                                    MessageType::UserMessage => (ui.style().visuals.text_color(), "💬"),
                                    MessageType::SystemInfo => (egui::Color32::LIGHT_BLUE, "ℹ️"),
                                    MessageType::SystemError => (egui::Color32::LIGHT_RED, "⚠️"),
                                    MessageType::ServerResponse => (egui::Color32::LIGHT_GREEN, "📢"),
                                    MessageType::GroupMessage { from: _, group: _ } => (egui::Color32::YELLOW, "👥"),
                                };
                                
                                ui.label(icon);
                                ui.colored_label(color, &msg.text);
                            });
                        }
                    }
                });
        });
    }
    
    fn show_help_ui(&mut self, ctx: &egui::Context) {
        egui::Window::new("❓ Ruggine Chat Help")
            .resizable(true)
            .default_width(500.0)
            .max_width(600.0)
            .show(ctx, |ui| {
                ui.heading("📚 Available Commands");
                ui.separator();
                
                ui.label("🔹 /register <username> - Register with a username");
                ui.label("🔹 /users - Show all online users");
                ui.label("🔹 /create_group <name> - Create a new group");
                ui.label("🔹 /my_groups - Show your groups");
                ui.label("🔹 /join_group <name> - Join a group");
                ui.label("🔹 /leave_group <name> - Leave a group");
                ui.label("🔹 /invite <user> <group> - Invite user to group");
                ui.label("🔹 /msg <group> <message> - Send message to group");
                ui.label("🔹 /quit - Disconnect from server");
                
                ui.separator();
                ui.heading("💡 Tips & Tricks");
                ui.label("• Use the sidebar panels for easy group and user management");
                ui.label("• Join a group to send messages without typing commands");
                ui.label("• Press Enter in the message box to send quickly");
                ui.label("• Use the menu bar for quick access to features");
                ui.label("• Toggle auto-scroll in the View menu");
                
                ui.separator();
                ui.heading("🎨 Interface Guide");
                ui.label("💬 Blue messages = System information");
                ui.label("⚠️ Red messages = Errors");
                ui.label("📢 Green messages = Server responses");
                ui.label("👥 Yellow messages = Group messages");
                
                ui.separator();
                if ui.button("❌ Close Help").clicked() {
                    self.show_help_dialog = false;
                }
            });
    }
    
    fn show_settings_ui(&mut self, ctx: &egui::Context) {
        egui::Window::new("⚙️ Settings")
            .resizable(false)
            .default_width(300.0)
            .show(ctx, |ui| {
                ui.heading("🎨 Appearance");
                ui.separator();
                
                ui.horizontal(|ui| {
                    ui.label("Theme:");
                    if ui.radio(self.dark_mode, "🌙 Dark").clicked() {
                        self.dark_mode = true;
                    }
                    if ui.radio(!self.dark_mode, "☀️ Light").clicked() {
                        self.dark_mode = false;
                    }
                });
                
                ui.horizontal(|ui| {
                    ui.label("Font Size:");
                    ui.add(egui::Slider::new(&mut self.message_font_size, 10.0..=20.0).suffix("px"));
                });
                
                ui.separator();
                ui.heading("🔧 Behavior");
                ui.checkbox(&mut self.auto_scroll, "Auto-scroll messages");
                
                ui.separator();
                if ui.button("❌ Close Settings").clicked() {
                    self.show_settings_dialog = false;
                }
            });
    }
}
