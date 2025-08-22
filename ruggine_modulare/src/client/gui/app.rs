use iced::{Application, Command, Element, Theme};
use crate::client::models::app_state::{AppState, ChatAppState};
use crate::client::models::messages::Message;
use crate::client::services::chat_service::ChatService;
use crate::client::gui::views;

pub struct ChatApp {
    pub state: ChatAppState,
    pub chat_service: ChatService,
}

impl Application for ChatApp {
    type Message = Message;
    type Theme = Theme;
    type Executor = iced::executor::Default;
    type Flags = ();

    fn new(_flags: ()) -> (Self, Command<Message>) {
        (
            ChatApp {
                state: ChatAppState::default(),
                chat_service: ChatService::default(),
            },
            Command::none(),
        )
    }

    fn title(&self) -> String {
        "Ruggine Chat - Modulare".to_string()
    }

    fn update(&mut self, message: Message) -> Command<Message> {
        // Delega la logica ai servizi e aggiorna lo stato
        self.state.update(message, &mut self.chat_service)
    }

    fn view(&self) -> Element<Message> {
        match &self.state.app_state {
            AppState::Registration => views::registration::view(&self.state),
            AppState::MainActions => views::main_actions::view(&self.state),
            AppState::PrivateChat(username) => views::private_chat::view(&self.state, username),
            AppState::GroupChat(group_id, group_name) => views::group_chat::view(&self.state, group_id, group_name),
            AppState::FriendRequests => views::friend_requests::view(&self.state),
            AppState::Chat => views::main_actions::view(&self.state),
        }
    }
}
