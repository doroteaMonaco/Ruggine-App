use iced::{Element, widget::{column, text}};
use crate::client::models::messages::Message;
use crate::client::models::app_state::ChatAppState;

pub fn view(_state: &ChatAppState, username: &str) -> Element<Message> {
    column![
        text(format!("Private Chat View (modulare) with {}", username))
    ].into()
}
