// src/app/state.rs
use crate::models::DiscordMessage;
use std::collections::HashMap;
use std::time::Instant;
use ratatui::widgets::ListState; // 🟢 ADDED: Tracking struct map bounds

pub struct AppState {
    pub token: String,
    pub target_channel_id: String,
    pub messages: Vec<DiscordMessage>,
    pub input_text: String,
    pub typing_users: HashMap<String, Instant>,
    pub list_state: ListState, // 🟢 ADDED: Structural state instance
}

impl AppState {
    pub fn new(token: String) -> Self {
        let mut list_state = ListState::default();
        list_state.select(Some(0)); // Start tracking focus indexes from ground layer zero

        Self {
            token,
            target_channel_id: String::new(),
            messages: Vec::new(),
            input_text: String::new(),
            typing_users: HashMap::new(),
            list_state, // 🟢 INITIALIZED
        }
    }
}
