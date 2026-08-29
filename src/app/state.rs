// src/app/state.rs
use crate::models::DiscordMessage;
use ratatui::widgets::ListState;

pub struct AppState {
    pub token: String,
    pub target_channel_id: String,
    pub messages: Vec<DiscordMessage>,
    pub input_text: String,
    pub list_state: ListState,
    pub failed_nonces: Vec<String>, // 🟢 ADDED: Remembers failed message IDs for instant retries
}

impl AppState {
    pub fn new(token: String) -> Self {
        let mut list_state = ListState::default();
        list_state.select(Some(0));

        Self {
            token,
            target_channel_id: String::new(),
            messages: Vec::new(),
            input_text: String::new(),
            list_state,
            failed_nonces: Vec::new(), // 🟢 INITIALIZED
        }
    }
}
