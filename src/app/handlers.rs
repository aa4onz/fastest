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
            AppEvent::MessageSent { nonce, timestamp } => {
                if let Some(m) = self.messages.iter_mut().find(|x| x.nonce == nonce) {
                    m.status = MessageStatus::Delivered;
                    m.timestamp = timestamp;
                    self.failed_nonces.retain(|x| x != &nonce);
                }
            }
            AppEvent::MessageFailed { nonce } => {
                // FIXED LOOKUP: Securely locate message using the locked nonce handle index code
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
                            self.retry_chat(&last_failed_nonce, tx, client);
                        }
                    }
                    KeyCode::Char(c) => self.input_text.push(c),
                    KeyCode::Backspace => {
                        self.input_text.pop();
                    }
                    KeyCode::Enter if !self.input_text.is_empty() => self.send_chat(tx, client),
                    _ => {}
                }
            }
            _ => {}
        }
        false
    }

    fn retry_chat(&mut self, nonce: &str, tx: &Sender<AppEvent>, client: &reqwest::Client) {
        let (text, found) = if let Some(m) = self.messages.iter_mut().find(|x| x.nonce == nonce) {
            m.status = MessageStatus::Sending;
            m.timestamp = format!("{} | ...", Local::now().format("%H:%M:%S")); // Reset display to loading status
            (m.content.clone(), true)
        } else {
            (String::new(), false)
        };

        if !found {
            return;
        }

        let cid = self.target_channel_id.clone();
        let nonce_clone = nonce.to_string();
        let (token, c, tx) = (self.token.clone(), client.clone(), tx.clone());
        let start_time = std::time::Instant::now();

        tokio::spawn(async move {
            let url = format!("https://discord.com/api/v10/channels/{}/messages", cid);
            let p = crate::models::MessagePayload {
                content: text,
                nonce: nonce_clone.clone(),
            };

            let res = c
                .post(&url)
                .header("Authorization", &token)
                .header("Content-Type", "application/json")
                .header("Accept", "*/*")
                .header("Origin", "https://discord.com")
                .header("X-Discord-Locale", "en-US")
                .json(&p)
                .send()
                .await;

            if let Ok(resp) = res {
                let status = resp.status();
                if status.is_success() {
                    let duration = start_time.elapsed().as_millis();
                    let combined_time_string = format!(
                        "{} | Sent in {}ms",
                        Local::now().format("%H:%M:%S"),
                        duration
                    );
                    let _ = tx
                        .send(AppEvent::MessageSent {
                            nonce: nonce_clone,
                            timestamp: combined_time_string,
                        })
                        .await;
                    return;
                } else {
                    let raw_err = resp.text().await.unwrap_or_else(|_| "Rejected".to_string());
                    let parsed_err = if let Some(idx) = raw_err.find("message\":") {
                        raw_err[idx + 9..]
                            .chars()
                            .filter(|c| *c != '"' && *c != '}' && *c != '{')
                            .take(80)
                            .collect::<String>()
                    } else {
                        raw_err
                            .chars()
                            .filter(|c| !c.is_control() && *c != '{' && *c != '}' && *c != '"')
                            .take(80)
                            .collect::<String>()
                    };
                    let combined_err_string = format!(
                        "{} | ❌ {}",
                        Local::now().format("%H:%M:%S"),
                        parsed_err.trim()
                    );

                    // FIXED STATE DISPATCH: Injects the locked tracking nonce so state tracking memory remains perfectly preserved
                    let _ = tx
                        .send(AppEvent::MessageSent {
                            nonce: nonce_clone.clone(),
                            timestamp: combined_err_string,
                        })
                        .await;
                    let _ = tx
                        .send(AppEvent::MessageFailed { nonce: nonce_clone })
                        .await;
                    return;
                }
            }
            let _ = tx
                .send(AppEvent::MessageFailed { nonce: nonce_clone })
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
            let p = crate::models::MessagePayload {
                content: text,
                nonce: nonce.clone(),
            };

            let res = c
                .post(&url)
                .header("Authorization", &token)
                .header("Content-Type", "application/json")
                .header("Accept", "*/*")
                .header("Origin", "https://discord.com")
                .header("X-Discord-Locale", "en-US")
                .json(&p)
                .send()
                .await;

            if let Ok(resp) = res {
                let status = resp.status();
                if status.is_success() {
                    let duration = start_time.elapsed().as_millis();
                    let combined_time_string = format!(
                        "{} | Sent in {}ms",
                        Local::now().format("%H:%M:%S"),
                        duration
                    );
                    let _ = tx
                        .send(AppEvent::MessageSent {
                            nonce,
                            timestamp: combined_time_string,
                        })
                        .await;
                    return;
                } else {
                    let raw_err = resp.text().await.unwrap_or_else(|_| "Rejected".to_string());
                    let parsed_err = if let Some(idx) = raw_err.find("message\":") {
                        raw_err[idx + 9..]
                            .chars()
                            .filter(|c| *c != '"' && *c != '}' && *c != '{')
                            .take(80)
                            .collect::<String>()
                    } else {
                        raw_err
                            .chars()
                            .filter(|c| !c.is_control() && *c != '{' && *c != '}' && *c != '"')
                            .take(80)
                            .collect::<String>()
                    };
                    let combined_err_string = format!(
                        "{} | ❌ {}",
                        Local::now().format("%H:%M:%S"),
                        parsed_err.trim()
                    );

                    // FIXED STATE DISPATCH: Injects the locked tracking nonce so state tracking memory remains perfectly preserved
                    let _ = tx
                        .send(AppEvent::MessageSent {
                            nonce: nonce.clone(),
                            timestamp: combined_err_string,
                        })
                        .await;
                    let _ = tx.send(AppEvent::MessageFailed { nonce }).await;
                    return;
                }
            }
            let _ = tx.send(AppEvent::MessageFailed { nonce }).await;
        });
    }
}
