// src/app/handlers.rs
use crate::models::{AppEvent, DiscordMessage, MessageStatus};
use chrono::Local;
use crossterm::event::{Event, KeyCode, KeyModifiers};
use tokio::sync::mpsc::Sender;

impl crate::app::state::AppState {
    pub async fn handle_event(
        &mut self,
        event: AppEvent,
        tx: &Sender<AppEvent>,
        client: &reqwest::Client,
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
            // 🟢 UPDATED: Captures the starting clock and calculates total loop turnarounds natively
            AppEvent::MessageSent { nonce, timestamp, enter_click_time } => {
                if let Some(m) = self.messages.iter_mut().find(|x| x.nonce == nonce) {
                    m.status = MessageStatus::Delivered;
                    
                    // Subtracts Enter button execution stamp from current system time
                    let true_turnaround_ms = enter_click_time.elapsed().as_millis();
                    m.timestamp = format!("{} | Total Loop: {}ms", timestamp, true_turnaround_ms);
                    
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
                            // Retries use fresh clocks to keep measurements accurate
                            let click_clock = std::time::Instant::now();
                            self.retry_chat(&last_failed_nonce, tx, client, click_clock);
                        }
                    }
                    KeyCode::Char(c) => {
                        if self.input_text.is_empty() {
                            let cid = self.target_channel_id.clone();
                            let token = self.token.clone();
                            let c_client = client.clone();
                            
                            tokio::spawn(async move {
                                let url = format!("https://discord.com/api/v10/channels/{}/typing", cid);
                                let _ = c_client.post(&url)
                                    .header("Authorization", &token)
                                    .header("Content-Type", "application/json")
                                    .header("Content-Length", "0")
                                    .send()
                                    .await;
                            });
                        }
                        self.input_text.push(c);
                    }
                    KeyCode::Backspace => {
                        self.input_text.pop();
                    }
                    // 🟢 THE STARTING POINT: Capture hardware time index the exact millisecond Enter matches
                    KeyCode::Enter if !self.input_text.is_empty() => {
                        let click_clock = std::time::Instant::now();
                        self.send_chat(tx, client, click_clock);
                    }
                    _ => {}
                }
            }
            _ => {}
        }
        false
    }

    fn retry_chat(&mut self, nonce: &str, tx: &Sender<AppEvent>, client: &reqwest::Client, click_clock: std::time::Instant) {
        let (text, found) = if let Some(m) = self.messages.iter_mut().find(|x| x.nonce == nonce) {
            m.status = MessageStatus::Sending;
            m.timestamp = format!("{} | ...", Local::now().format("%H:%M:%S"));
            (m.content.clone(), true)
        } else {
            (String::new(), false)
        };

        if !found { return; }

        let cid = self.target_channel_id.clone();
        let nonce_clone = nonce.to_string();
        let (token, c, tx) = (self.token.clone(), client.clone(), tx.clone());
        let start_time = std::time::Instant::now();

        tokio::spawn(async move {
            let url = format!("https://discord.com/api/v10/channels/{}/messages", cid);
            let p = crate::models::MessagePayload { content: text, nonce: nonce_clone.clone() };

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
                    let clean_time = format!("API: {}ms", duration);
                    let _ = tx.send(AppEvent::MessageSent {
                        nonce: nonce_clone,
                        timestamp: clean_time,
                        enter_click_time: click_clock,
                    }).await;
                    return;
                }
            }
            let _ = tx.send(AppEvent::MessageFailed { nonce: nonce_clone }).await;
        });
    }

    fn send_chat(&mut self, tx: &Sender<AppEvent>, client: &reqwest::Client, click_clock: std::time::Instant) {
        let text = std::mem::take(&mut self.input_text);
        let cid = self.target_channel_id.clone();
        
        let random_salt = rand::random::<u64>();
        let nonce = format!("n-{}-{}", Local::now().timestamp_nanos_opt().unwrap_or(0), random_salt);
        let current_time_str = Local::now().format("%H:%M:%S").to_string();

        self.messages.push(DiscordMessage {
            nonce: nonce.clone(),
            author: "You".to_string(),
            content: text.clone(),
            timestamp: format!("{} | ...", current_time_str),
            status: MessageStatus::Sending,
        });

        let (token, c, tx) = (self.token.clone(), client.clone(), tx.clone());
        let start_time = std::time::Instant::now();

        tokio::spawn(async move {
            let url = format!("https://discord.com{}/messages", cid);
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
                    let clean_time = format!("API: {}ms", duration);
                    
                    // 🚀 Passes both the raw API speed and your keyboard click time tracker back to the engine
                    let _ = tx.send(AppEvent::MessageSent {
                        nonce,
                        timestamp: clean_time,
                        enter_click_time: click_clock,
                    }).await;
                    return;
                }
            }
            let _ = tx.send(AppEvent::MessageFailed { nonce }).await;
        });
    }
}
