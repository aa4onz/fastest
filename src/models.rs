// src/models.rs
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone)]
pub enum MessageStatus { Sending, Delivered, Failed }

#[derive(Debug, Clone)]
pub struct DiscordMessage {
    pub nonce: String,
    pub author: String,
    pub content: String,
    pub timestamp: String,
    pub status: MessageStatus,
}

#[derive(Debug, Clone)]
pub enum AppEvent {
    IncomingMessage(DiscordMessage),
    MessageSent { nonce: String, timestamp: String },
    MessageFailed { nonce: String },
    Terminal(crossterm::event::Event),
    GatewayClosed,
}

#[derive(Serialize)]
pub struct MessagePayload {
    pub content: String,
    pub nonce: String,
}

#[derive(Deserialize)]
pub struct GatewayPayload {
    pub op: u8,
    #[serde(default)]
    pub d: serde_json::Value,
    #[serde(default)]
    pub t: Option<String>,
}
