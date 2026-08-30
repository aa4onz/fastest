// src/app/state.rs
use crate::models::DiscordMessage;
use ratatui::widgets::ListState;
use std::time::Instant;

pub struct AppState {
    pub token: String,
    pub target_channel_id: String,
    pub messages: Vec<DiscordMessage>,
    pub input_text: String,
    pub list_state: ListState,
    pub failed_nonces: Vec<String>,
    pub last_typing_sent: Option<Instant>, // 🟢 Tracks the cooldown window
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
            failed_nonces: Vec::new(),
            last_typing_sent: None, // 🟢 Starts clean
        }
    }
}
