// src/app/state.rs
use crate::models::{ActivePanel, Channel, DiscordMessage, MessageStatus, Server};
use chrono::Local;
use ratatui::widgets::ListState;
use std::{collections::HashMap, io, path::Path, time::Instant};

pub struct AppState {
    pub servers: Vec<Server>,
    pub selected_server_idx: usize,
    pub selected_channel_idx: usize,
    pub servers_state: ListState,
    pub channels_state: ListState,
    pub input_text: String,
    pub active_panel: ActivePanel,
    pub token: String,
    pub my_user_id: String,
    pub username: String,
    pub typing_users: HashMap<String, HashMap<String, Instant>>,
    pub messages: Vec<DiscordMessage>,
}

impl AppState {
    pub fn new(token: String) -> Self {
        let mut servers_state = ListState::default();
        servers_state.select(Some(0));
        let mut channels_state = ListState::default();
        channels_state.select(Some(0));

        Self {
            servers: vec![Server {
                id: "0".to_string(),
                name: "Loading Guilds...".to_string(),
                channels: vec![Channel { id: "0".to_string(), name: "please-wait".to_string() }],
            }],
            selected_server_idx: 0,
            selected_channel_idx: 0,
            servers_state,
            channels_state,
            input_text: String::new(),
            active_panel: ActivePanel::ChatInput,
            token,
            my_user_id: String::new(),
            username: String::new(),
            typing_users: HashMap::new(),
            messages: vec![DiscordMessage {
                nonce: "system".to_string(),
                author: "System".to_string(),
                content: "Zero-latency core online.".to_string(),
                timestamp: Local::now().format("%H:%M:%S").to_string(),
                status: MessageStatus::Delivered,
            }],
        }
    }

    pub fn get_cached_or_prompt_token() -> Result<String, io::Error> {
        let cache_path = Path::new(".token_cache");
        if cache_path.exists() {
            let token = std::fs::read_to_string(cache_path)?.trim().to_string();
            if !token.is_empty() { return Ok(token); }
        }
        println!("Please enter your Discord Token: ");
        let mut token_input = String::new();
        io::stdin().read_line(&mut token_input)?;
        Ok(token_input.trim().to_string())
    }

    pub fn current_channel_id(&self) -> String {
        self.servers.get(self.selected_server_idx)
            .and_then(|s| s.channels.get(self.selected_channel_idx))
            .map(|c| c.id.clone())
            .unwrap_or_default()
    }

    pub fn current_channel_name(&self) -> String {
        self.servers.get(self.selected_server_idx)
            .and_then(|s| s.channels.get(self.selected_channel_idx))
            .map(|c| c.name.clone())
            .unwrap_or_else(|| "none".to_string())
    }
}
