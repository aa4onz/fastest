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
                    
                    // TOTAL-TOTAL LATENCY CALCULATOR: Extracts the original keystroke timestamp
                    if let Some(stripped_nonce) = nonce.strip_prefix("n-") {
                        if let Ok(creation_nanos) = stripped_nonce.parse::<i64>() {
                            let current_nanos = chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0);
                            let total_absolute_diff_ms = (current_nanos - creation_nanos) / 1_000_000;
                            
                            // Displays your absolute true real-world lag covering the entire global trip
                            m.timestamp = format!(
                                "{} | Total True {}ms",
                                Local::now().format("%H:%M:%S"),
                                total_absolute_diff_ms
                            );
                        }
                    } else {
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
                            self.retry_chat(&last_failed_nonce, tx, client);
                        }
                    }
                    KeyCode::Char(c) => {
                        self.input_text.push(c);
                        self.trigger_typing_indicator(client);
                    }
                    KeyCode::Backspace => {
                        self.input_text.pop();
                        self.trigger_typing_indicator(client);
                    }
                    KeyCode::Enter if !self.input_text.is_empty() => {
                        // Reset typing indicator immediately so next typing session triggers instantly
                        self.last_typing_sent = None;
                        self.send_chat(tx, client);
                    }
                    _ => {}
                }
            }
            _ => {}
        }
        false
    }

    fn trigger_typing_indicator(&mut self, client: &reqwest::Client) {
        if self.target_channel_id.is_empty() {
            return;
        }

        // Throttle indicator requests to once every 7 seconds
        if let Some(last_sent) = self.last_typing_sent {
            if last_sent.elapsed() < std::time::Duration::from_secs(7) {
                return;
            }
        }

        self.last_typing_sent = Some(std::time::Instant::now());

        let cid = self.target_channel_id.clone();
        let token = self.token.clone();
        let c = client.clone();

        tokio::spawn(async move {
            let url = format!("https://discord.com/api/v10/channels/{}/typing", cid);
            let _ = c
                .post(&url)
                .header("Authorization", &token)
                .header("Content-Length", "0")
                .send()
                .await;
        });
    }

    fn retry_chat(&mut self, nonce: &str, tx: &Sender<AppEvent>, client: &reqwest::Client) {
        let (text, found) = if let Some(m) = self.messages.iter_mut().find(|x| x.nonce == nonce) {
            m.status = MessageStatus::Sending;
            m.timestamp = format!("{} | ...", Local::now().format("%H:%M:%S"));
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

        tokio::spawn(async move {
            let url = format!("https://discord.com/api/v10/channels/{}/messages", cid);
            
            let raw_body = [
                b"{"content":"", text.as_bytes(), 
                b"","nonce":"", nonce_clone.as_bytes(), 
                b""}"
            ].concat();

            let res = c
                .post(&url)
                .header("Authorization", &token)
                .header("Content-Type", "application/json")
                .header("Accept", "*/*")
                .header("Origin", "https://discord.com")
                .header("X-Discord-Locale", "en-US")
                .body(raw_body)
                .send()
                .await;

            if let Ok(resp) = res {
                let status = resp.status();
                if status.is_success() {
                    let _ = tx
                        .send(AppEvent::MessageSent {
                            nonce: nonce_clone,
                            timestamp: String::new(),
                        })
                        .await;
                    return;
                } else {
                    let raw_err = resp.text().await.unwrap_or_else(|_| "Rejected".to_string());
                    let parsed_err = if let Some(idx) = raw_err.find("message\":") {
                        raw_err[idx + 9..]
                            .chars()
                            //.filter(|c| *c != '"' && *c != '}' && *c != '{')
                            .filter(|c| *c != '"' && *c != '}' && *c != '{')
                            .take(80)
                            .collect::<String>()
                    } else {
                        raw_err
                            .chars()
                            //.filter(|c| !c.is_control() && *c != '{' && *c != '}' && *c != '"')
                            .filter(|c| !c.is_control() && *c != '{' && *c != '}' && *c != '"')
                            .take(80)
                            .collect::<String>()
                    };
                    let combined_err_string = format!(
                        "{} | ❌ {}",
                        Local::now().format("%H:%M:%S"),
                        parsed_err.trim()
                    );

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
        
        // CRUCIAL: Encodes the exact UTC nanosecond clock data directly into the message tracking string
        let nonce = format!("n-{}", chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0));
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
            
            let raw_body = [
                b"{"content":"", text.as_bytes(), 
                b"","nonce":"", nonce.as_bytes(), 
                b""}"
            ].concat();

            let res = c
                .post(&url)
                .header("Authorization", &token)
                .header("Content-Type", "application/json")
                .header("Accept", "*/*")
                .header("Origin", "https://discord.com")
                .header("X-Discord-Locale", "en-US")
                .body(raw_body)
                .send()
                .await;

            if let Ok(resp) = res {
                let status = resp.status();
                if status.is_success() {
                    let _ = tx
                        .send(AppEvent::MessageSent {
                            nonce,
                            timestamp: String::new(),
                        })
                        .await;
                    return;
                } else {
                    let raw_err = resp.text().await.unwrap_or_else(|_| "Rejected".to_string());
                    let parsed_err = if let Some(idx) = raw_err.find("message\":") {
                        raw_err[idx + 9..]
                            .chars()
                            //.filter(|c| *c != '"' && *c != '}' && *c != '{')
                            .filter(|c| *c != '"' && *c != '}' && *c != '{')
                            .take(80)
                            .collect::<String>()
                    } else {
                        raw_err
                            .chars()
                            //.filter(|c| !c.is_control() && *c != '{' && *c != '}' && *c != '"')
                            .filter(|c| !c.is_control() && *c != '{' && *c != '}' && *c != '"')
                            .take(80)
                            .collect::<String>()
                    };
                    let combined_err_string = format!(
                        "{} | ❌ {}",
                        Local::now().format("%H:%M:%S"),
                        parsed_err.trim()
                    );

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
