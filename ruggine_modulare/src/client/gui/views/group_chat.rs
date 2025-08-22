use iced::{Element, widget::{column, text}};
use crate::client::models::messages::Message;
use crate::client::models::app_state::ChatAppState;

pub fn view(_state: &ChatAppState, group_id: &str, group_name: &str) -> Element<Message> {
    column![
        text(format!("Group Chat View (modulare) for {} ({})", group_name, group_id))
    ].into()
}
