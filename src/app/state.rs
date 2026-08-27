// src/app/state.rs
use crate::models::{DiscordMessage, MessageStatus};

pub struct AppState {
    pub token: String,
    pub target_channel_id: String,
    pub messages: Vec<DiscordMessage>,
    pub input_text: String,
}
