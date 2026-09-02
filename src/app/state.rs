// src/app/state.rs
use crate::models::{AppEvent, DiscordMessage, MessageStatus};
use chrono::Local;
use crossterm::event::{Event, KeyCode, KeyModifiers};
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
    pub last_typing_sent: Option<Instant>, // 🟢 Tracks the cooldown window
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
            last_typing_sent: None, // 🟢 Starts clean
        }
    }

    pub async fn handle_event(
        &mut self,
        event: AppEvent,
        tx: &Sender<AppEvent>,
    ) -> bool {
        match event {
            AppEvent::IncomingMessage(m) => {
                if m.nonce.starts_with("err-") {
                    self.messages.push(m);
                } else if !self
                    .messages
                    .iter()
                    .any(|x| x.nonce == m.nonce && !m.nonce.is_empty())
                {
                    self.messages.push(m);
                }
            }
            AppEvent::MessageSent { nonce, timestamp } => {
                if let Some(m) = self.messages.iter_mut().find(|x| x.nonce == nonce) {
                    m.status = MessageStatus::Delivered;
                    
                    if let Some(stripped_nonce) = nonce.strip_prefix("n-") {
                        if let Ok(creation_nanos) = stripped_nonce.parse::<i64>() {
                            let current_nanos = chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0);
                            let total_absolute_diff_ms = (current_nanos - creation_nanos) / 1_000_000;
                            
                            m.timestamp = format!(
                                "{} | Total True {}ms",
                                Local::now().format("%H:%M:%S"),
                                total_absolute_diff_ms
                            );
                        }
                    } else if !timestamp.is_empty() {
                        m.timestamp = timestamp;
                    }
                    
                    self.failed_nonces.retain(|x| x != &nonce);
                }
            }
            AppEvent::MessageFailed { nonce } => {
                if let Some(m) = self.messages.iter_mut().find(|x| x.nonce == nonce) {
                    m.status = MessageStatus::Failed;
                }
                if !self.failed_nonces.contains(&nonce) {
                    self.failed_nonces.push(nonce);
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
            AppEvent::Terminal(Event::Key(k))
                if k.kind == crossterm::event::KeyEventKind::Press =>
            {
                if k.code == KeyCode::Char('c') && k.modifiers.contains(KeyModifiers::CONTROL) {
                    return true;
                }

                match k.code {
                    KeyCode::Tab => {
                        if let Some(last_failed_nonce) = self.failed_nonces.last().cloned() {
                            let (text, found) = if let Some(m) = self.messages.iter_mut().find(|x| x.nonce == last_failed_nonce) {
                                m.status = MessageStatus::Sending;
                                m.timestamp = format!("{} | ...", Local::now().format("%H:%M:%S"));
                                (m.content.clone(), true)
                            } else {
                                (String::new(), false)
                            };

                            if found {
                                let _ = tx.send(AppEvent::HttpSendChat {
                                    nonce: last_failed_nonce,
                                    text,
                                }).await;
                            }
                        }
                    }
                    KeyCode::Char(c) => {
                        self.input_text.push(c);
                        
                        // Local instant check triggers channel emit without UI thread calculation stalls
                        let trigger = match self.last_typing_sent {
                            Some(last_sent) => last_sent.elapsed() >= std::time::Duration::from_secs(8),
                            None => true,
                        };

                        if trigger && !self.target_channel_id.is_empty() {
                            self.last_typing_sent = Some(std::time::Instant::now());
                            let _ = tx.send(AppEvent::HttpTriggerTyping).await;
                        }
                    }
                    KeyCode::Backspace => {
                        self.input_text.pop();
                    }
                    KeyCode::Enter if !self.input_text.is_empty() => {
                        self.last_typing_sent = None;
                        let text = std::mem::take(&mut self.input_text);
                        let nonce = format!("n-{}", chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0));
                        let current_time_str = Local::now().format("%H:%M:%S").to_string();

                        self.messages.push(DiscordMessage {
                            nonce: nonce.clone(),
                            author: "You".to_string(),
                            content: text.clone(),
                            timestamp: format!("{} | ...", current_time_str),
                            status: MessageStatus::Sending,
                        });

                        let _ = tx.send(AppEvent::HttpSendChat { nonce, text }).await;
                    }
                    _ => {}
                }
            }
            _ => {}
        }
        false
    }
}
