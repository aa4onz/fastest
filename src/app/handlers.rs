// src/app/handlers.rs
use crate::models::{AppEvent, DiscordMessage, MessageStatus};
use chrono::Local;
use crossterm::event::{Event, KeyCode, KeyModifiers};
use tokio::sync::mpsc::Sender;

impl crate::app::state::AppState {
    pub async fn handle_event(&mut self, event: AppEvent, tx: &Sender<AppEvent>, client: &reqwest::Client) -> bool {
        match event {
            AppEvent::IncomingMessage(m) => {
                // FIXED: Force errors starting with 'err-' to always bypass filters and print on screen
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
                    KeyCode::Enter if !self.input_text.is_empty() => self.send_chat(tx, client),
                    _ => {}
                }
            }
            _ => {}
        }
        false
    }

    fn send_chat(&mut self, tx: &Sender<AppEvent>, client: &reqwest::Client) {
        let text = std::mem::take(&mut self.input_text);
        let cid = self.target_channel_id.clone();
        let nonce = format!("n-{}", Local::now().timestamp_nanos_opt().unwrap_or(0));

        self.messages.push(DiscordMessage {
            nonce: nonce.clone(),
            author: "You".to_string(),
            content: text.clone(),
            timestamp: Local::now().format("%H:%M:%S").to_string(),
            status: MessageStatus::Sending,
        });

        let (token, c, tx) = (self.token.clone(), client.clone(), tx.clone());
        tokio::spawn(async move {
            // FIXED: Fully formed endpoint URL template targeting the v10 channels rest schema
            let url = format!("https://discord.com/api/v10/channels/{}/messages", cid);
            let p = crate::models::MessagePayload { content: text, nonce: nonce.clone() };
            
            let res = c.post(&url)
                .header("Authorization", &token)
                .header("Content-Type", "application/json")
                .header("Accept", "*/*")
                .header("Origin", "https://discord.com")
                .header("X-Discord-Locale", "en-US")
                .json(&p)
                .send()
                .await;

            match res {
                Ok(resp) => {
                    let status = resp.status();
                    if status.is_success() {
                        let _ = tx.send(AppEvent::MessageSent { nonce, timestamp: Local::now().format("%H:%M:%S").to_string() }).await;
                    } else {
                        let raw_body = resp.text().await.unwrap_or_else(|_| "No payload body details available".to_string());
                        let _ = tx.send(AppEvent::IncomingMessage(DiscordMessage {
                            nonce: format!("err-{}", nonce),
                            author: format!("❌ SERVER REJECT ({})", status),
                            content: format!("Reason: {}", raw_body.chars().take(80).collect::<String>()),
                            timestamp: Local::now().format("%H:%M:%S").to_string(),
                            status: MessageStatus::Failed,
                        })).await;
                        let _ = tx.send(AppEvent::MessageFailed { nonce }).await;
                    }
                }
                Err(e) => {
                    let _ = tx.send(AppEvent::IncomingMessage(DiscordMessage {
                        nonce: format!("err-{}", nonce),
                        author: "❌ NETWORK ROUTE ERROR".to_string(),
                        content: format!("Dropped: {}", e),
                        timestamp: Local::now().format("%H:%M:%S").to_string(),
                        status: MessageStatus::Failed,
                    })).await;
                    let _ = tx.send(AppEvent::MessageFailed { nonce }).await;
                }
            }
        });
    }
}
