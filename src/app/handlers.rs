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
                
                // Notifies Discord's backend API that YOU are currently typing
                self.trigger_outbound_typing(client);

                match k.code {
                    KeyCode::Char(c) => self.input_text.push(c),
                    KeyCode::Backspace => { self.input_text.pop(); }
                    KeyCode::Enter if !self.input_text.is_empty() => self.send_chat(tx, client),
                    _ => {}
                }
            }
            // FIXED: Added the catch-all arm to safely ignore unhandled window focus/mouse adjustments
            _ => {}
        }
        false
    }

    fn trigger_outbound_typing(&mut self, client: &reqwest::Client) {
        static mut LAST_TYPING_TIME: Option<std::time::Instant> = None;
        unsafe {
            if let Some(last) = LAST_TYPING_TIME {
                if last.elapsed().as_secs() < 4 { return; }
            }
            LAST_TYPING_TIME = Some(std::time::Instant::now());
        }

        let cid = self.target_channel_id.clone();
        let token = self.token.clone();
        let c = client.clone();

        tokio::spawn(async move {
            let url = format!("https://discord.com/api/v10/channels/{}/typing", cid);
            let _ = c.post(&url)
                .header("Authorization", &token)
                .header("Content-Length", "0")
                .send()
                .await;
        });
    }

    fn send_chat(&mut self, tx: &Sender<AppEvent>, client: &reqwest::Client) {
        let text = std::mem::take(&mut self.input_text);
        let cid = self.target_channel_id.clone();
        let nonce = format!("n-{}", Local::now().timestamp_nanos_opt().unwrap_or(0));
        
        let start_time = std::time::Instant::now();
        let current_time_str = Local::now().format("%H:%M:%S").to_string();

        self.messages.push(DiscordMessage {
            nonce: nonce.clone(),
            author: "You".to_string(),
            content: text.clone(),
            timestamp: format!("{} | ...", current_time_str),
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

            if let Ok(resp) = res {
                if resp.status().is_success() {
                    let duration = start_time.elapsed().as_millis();
                    let combined_time_string = format!("{} | Sent in {}ms", Local::now().format("%H:%M:%S"), duration);
                    let _ = tx.send(AppEvent::MessageSent { nonce, timestamp: combined_time_string }).await;
                    return;
                }
            }
            let _ = tx.send(AppEvent::MessageFailed { nonce }).await;
        });
    }
}
