#[derive(Debug, Clone)]
pub enum AppState {
    Registration,
    MainActions,
    Chat,
    PrivateChat(String),
    GroupChat(String, String),
    FriendRequests,
}

#[derive(Debug, Clone, Default)]
pub struct ChatAppState {
    pub app_state: AppState,
    pub username: String,
    // ... altri campi di stato
}

impl ChatAppState {
    pub fn update(&mut self, _message: crate::client::models::messages::Message, _chat_service: &mut crate::client::services::chat_service::ChatService) -> iced::Command<crate::client::models::messages::Message> {
        // TODO: logica di update modulare
        iced::Command::none()
    }
}
