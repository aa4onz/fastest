// src/app/state.rs
use crate::models::DiscordMessage;

pub struct AppState {
    pub token: String,
    pub target_channel_id: String,
    pub messages: Vec<DiscordMessage>,
    pub input_text: String,
}

impl AppState {
    // FIXED: Added the required initialization method for main.rs
    pub fn new(token: String) -> Self {
        Self {
            token,
            target_channel_id: String::new(),
            messages: Vec::new(),
            input_text: String::new(),
        }
    }
}
