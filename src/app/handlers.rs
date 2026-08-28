// src/app/handlers.rs
use crate::models::{AppEvent, DiscordMessage, MessageStatus};
use chrono::Local;
use crossterm::event::{Event, KeyCode, KeyModifiers};
use tokio::sync::mpsc::Sender;

impl crate::app::state::AppState {
    pub async fn handle_event(&mut self, event: AppEvent, tx: &Sender<AppEvent>, _client: &reqwest::Client) -> bool {
        match event {
            AppEvent::IncomingMessage(m) => {
                if m.nonce.starts_with("err-") {
                    self.messages.push(m);
                } else if !self.messages.iter().any(|x| x.nonce == m.nonce && !m.nonce.is_empty()) {
                    self.messages.push(m);
                }
            }
            AppEvent::MessageSent { nonce, timestamp } => {
                if let Some(m) = self.messages.iter_mut().find(|x| x.nonce == nonce) {
                    m.status = MessageStatus::Delivered;
                    m.timestamp = timestamp;
                }
            }
            AppEvent::MessageFailed { nonce } => {
                if let Some(m) = self.messages.iter_mut().find(|x| x.nonce == nonce) {
                    m.status = MessageStatus::Failed;
                }
            }
            AppEvent::GatewayClosed => {
                self.messages.push(DiscordMessage {
                    nonce: "err-close".into(),
                    author: "System".into(),
                    content: "⚠️ WebSocket closed. Reconnecting...".into(),
                    timestamp: Local::now().format("%H:%M:%S").to_string(),
                    status: MessageStatus::Failed,
                });
            }
            AppEvent::Terminal(Event::Key(k)) if k.kind == crossterm::event::KeyEventKind::Press => {
                if k.code == KeyCode::Char('c') && k.modifiers.contains(KeyModifiers::CONTROL) { return true; }
                
                match k.code {
                    KeyCode::Char(c) => self.input_text.push(c),
                    KeyCode::Backspace => { self.input_text.pop(); }
                    KeyCode::Enter if !self.input_text.is_empty() => self.send_chat(tx),
                    _ => {}
                }
            }
            _ => {}
        }
        false
    }

    fn send_chat(&mut self, tx: &Sender<AppEvent>) {
        let text = std::mem::take(&mut self.input_text);
        let cid = self.target_channel_id.clone();
        let nonce = format!("n-{}", Local::now().timestamp_nanos_opt().unwrap_or(0));

        self.messages.push(DiscordMessage {
            nonce: nonce.clone(),
            author: "You".to_string(),
            content: text.clone(),
            timestamp: "...".to_string(),
            status: MessageStatus::Sending,
        });

        // FAST BYPASS: Throw the text parameters directly into the live background socket pipe
        let _ = tx.try_send(AppEvent::OutgoingMessageData {
            channel_id: cid,
            content: text,
            nonce,
        });
    }
}
