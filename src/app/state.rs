// src/app/state.rs
use crate::models::{DiscordMessage, AppEvent, MessageStatus};
use ratatui::widgets::ListState;
use std::time::Instant;
use tokio::sync::mpsc::Sender;

pub struct AppState {
    pub token: String,
    pub target_channel_id: String,
    pub messages: Vec<DiscordMessage>,
    pub input_text: String,
    pub list_state: ListState,
    pub failed_nonces: Vec<String>,
    pub last_typing_sent: Option<Instant>,
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
            last_typing_sent: None,
        }
    }

    /// ⚡ Handle keyboard interactions manually with zero extra buffering delays
    pub async fn handle_event(&mut self, event: AppEvent, tx: &Sender<AppEvent>) -> bool {
        match event {
            AppEvent::Terminal(crossterm::event::Event::Key(key_event)) => {
                // Only process when key is pressed down (ignores OS repeating frames)
                if key_event.kind == crossterm::event::KeyEventKind::Press {
                    match key_event.code {
                        // 1. Manually add typed characters to your text container
                        crossterm::event::KeyCode::Char(c) => {
                            self.input_text.push(c);
                            
                            // Triggers Discord's "typing..." status if it has been more than 4 seconds since the last one
                            let now = Instant::now();
                            if self.last_typing_sent.map_or(true, |last| now.duration_since(last).as_secs() >= 4) {
                                self.last_typing_sent = Some(now);
                                let _ = tx.send(AppEvent::HttpTriggerTyping).await;
                            }
                        }
                        // 2. Erase characters manually
                        crossterm::event::KeyCode::Backspace => {
                            self.input_text.pop();
                        }
                        // 3. Clear text fields if you hit Escape
                        crossterm::event::KeyCode::Esc => {
                            return true; // Exits app gracefully
                        }
                        // 4. 🚀 THE LAUNCHPAD: Hand over manually typed text instantly when hitting Enter
                        crossterm::event::KeyCode::Enter => {
                            if !self.input_text.is_empty() {
                                let manual_content = self.input_text.clone();
                                self.input_text.clear(); // Instantly empties text box for your next number

                                // Create a unique tracking identifier for this specific packet
                                let nonce = uuid::Uuid::new_v4().to_string();

                                // Pre-render your message locally so your app screen reflects your entry at 0ms
                                self.messages.push(DiscordMessage {
                                    author: "You".to_string(),
                                    content: manual_content.clone(),
                                    timestamp: chrono::Local::now().format("%H:%M:%S").to_string(),
                                    status: MessageStatus::Sending,
                                    nonce: nonce.clone(),
                                });

                                // Offload the outbound POST task out of the UI engine down the network pipeline
                                let _ = tx.send(AppEvent::HttpSendChat {
                                    nonce,
                                    text: manual_content,
                                }).await;
                            }
                        }
                        _ => {}
                    }
                }
            }
            // Update local view states when network worker triggers delivery success confirmations
            AppEvent::MessageSent { nonce, timestamp } => {
                if let Some(msg) = self.messages.iter_mut().find(|m| m.nonce == nonce) {
                    msg.status = MessageStatus::Delivered;
                    if !timestamp.is_empty() {
                        msg.timestamp = timestamp;
                    }
                }
            }
            // Update status indicator visually to [❌] if Discord rejects message parameters
            AppEvent::MessageFailed { nonce } => {
                if let Some(msg) = self.messages.iter_mut().find(|m| m.nonce == nonce) {
                    msg.status = MessageStatus::Failed;
                }
                self.failed_nonces.push(nonce);
            }
            _ => {}
        }
        false
    }
}
