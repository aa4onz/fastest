// src/app/handlers.rs
use crate::models::{AppEvent, DiscordMessage, MessageStatus};
use chrono::Local;
use crossterm::event::{Event, KeyCode, KeyModifiers};
use tokio::sync::mpsc::Sender;

impl crate::app::state::AppState {
    pub async fn handle_event(&mut self, event: AppEvent, tx: &Sender<AppEvent>, client: &reqwest::Client) -> bool {
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
                    // FIXED: Display the exact transmission RTT directly in the chat layout line
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
        
        // Performance Tracker: Store high-precision start timestamp
        let start_time = std::time::Instant::now();

        self.messages.push(DiscordMessage {
            nonce: nonce.clone(),
            author: "You".to_string(),
            content: text.clone(),
            timestamp: "...".to_string(),
            status: MessageStatus::Sending,
        });

        let (token, c, tx) = (self.token.clone(), client.clone(), tx.clone());
        tokio::spawn(async move {
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
                    if resp.status().is_success() {
                        // Performance Tracker: Calculate total round-trip sending latency
                        let duration = start_time.elapsed().as_millis();
                        let time_str = format!("Sent in {}ms", duration);
                        let _ = tx.send(AppEvent::MessageSent { nonce, timestamp: time_str }).await;
                    } else {
                        let raw_body = resp.text().await.unwrap_or_else(|_| "No payload body details available".to_string());
                        let _ = tx.send(AppEvent::IncomingMessage(DiscordMessage {
                            nonce: format!("err-{}", nonce),
                            author: "❌ SERVER REJECT".to_string(),
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
