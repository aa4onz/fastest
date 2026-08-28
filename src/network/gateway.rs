// src/network/gateway.rs
use crate::app::state::AppState;
use crate::models::{AppEvent, DiscordMessage, GatewayPayload, MessageStatus};
use futures_util::{SinkExt, StreamExt};
use std::sync::Arc;
use tokio::sync::{mpsc, mpsc::Sender, Mutex};
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};

pub async fn run_gateway_loop(app_state: Arc<Mutex<AppState>>, event_tx: Sender<AppEvent>) {
    let url = "wss://gateway.discord.gg/?v=10&encoding=json";
    loop {
        let (token, target_cid) = {
            let state = app_state.lock().await;
            (state.token.clone(), state.target_channel_id.clone())
        };
        if token.is_empty() { break; }

        if let Ok((ws, _)) = connect_async(url).await {
            let (mut write, mut read) = ws.split();
            
            if let Some(Ok(Message::Text(t))) = read.next().await {
                if let Ok(p) = serde_json::from_str::<GatewayPayload>(&t) {
                    if p.op == 10 {
                        let interval = p.d["heartbeat_interval"].as_u64().unwrap_or(41250);
                        let id = serde_json::json!({"op": 2, "d": {"token": token.clone(), "properties": {"$os": "windows", "$browser": "chrome", "$device": "pc"}}});
                        let _ = write.send(Message::Text(id.to_string())).await;
                        
                        let state_clone = Arc::clone(&app_state);
                        let shared_w = Arc::new(Mutex::new(write));
                        let h_write = Arc::clone(&shared_w);
                        
                        // Heartbeat Daemon Thread Task
                        tokio::spawn(async move {
                            loop {
                                tokio::time::sleep(std::time::Duration::from_millis(interval)).await;
                                if state_clone.lock().await.token.is_empty() { break; }
                                if h_write.lock().await.send(Message::Text(serde_json::json!({"op": 1, "d": null}).to_string())).await.is_err() { break; }
                            }
                        });

                        // FIXED: Initialize a true Receiver (cmd_rx) to handle keyboard text data safely
                        let (cmd_tx, mut cmd_rx) = mpsc::channel::<AppEvent>(100);
                        let w_tx = event_tx.clone();
                        let shared_w_clone = Arc::clone(&shared_w);
                        let state_ref = Arc::clone(&app_state);
                        
                        // Listeners for processing key entries from handers.rs
                        tokio::spawn(async move {
                            let mut start_time = std::time::Instant::now();
                            while let Some(event) = cmd_rx.recv().await {
                                if let AppEvent::OutgoingMessageData { channel_id, content, nonce } = event {
                                    start_time = std::time::Instant::now();
                                    let outbound_frame = serde_json::json!({
                                        "op": 8, 
                                        "d": {
                                            "channel_id": channel_id,
                                            "content": content,
                                            "nonce": nonce
                                        }
                                    });
                                    if shared_w_clone.lock().await.send(Message::Text(outbound_frame.to_string())).await.is_ok() {
                                        let duration = start_time.elapsed().as_millis();
                                        let _ = w_tx.send(AppEvent::MessageSent { nonce, timestamp: format!("Sent in {}ms", duration) }).await;
                                    } else {
                                        let _ = w_tx.send(AppEvent::MessageFailed { nonce }).await;
                                    }
                                }
                            }
                        });

                        // Intercept and route outbound text triggers to cmd_tx
                        let outbound_tx = cmd_tx.clone();
                        let local_event_tx = event_tx.clone();

                        // Real-time Event Receiver Processing Loop
                        while let Some(Ok(Message::Text(msg_text))) = read.next().await {
                            if let Ok(pay) = serde_json::from_str::<GatewayPayload>(&msg_text) {
                                if pay.op == 0 {
                                    let ev = pay.t.as_deref().unwrap_or("");
                                    
                                    if ev == "MESSAGE_CREATE" && pay.d["channel_id"].as_str() == Some(&target_cid) {
                                        let msg_id_str = pay.d["id"].as_str().unwrap_or("0");
                                        let transit_time_str = if let Ok(msg_id) = msg_id_str.parse::<u64>() {
                                            let discord_epoch_ms = (msg_id >> 22) + 1420070400000;
                                            let current_ms = chrono::Utc::now().timestamp_millis() as u64;
                                            format!("Recv latency: {}ms", current_ms.saturating_sub(discord_epoch_ms))
                                        } else {
                                            "Recv".to_string()
                                        };

                                        let nonce = pay.d["nonce"].as_str().unwrap_or("").to_string();
                                        let state = state_ref.lock().await;
                                        let is_dup = state.messages.iter().any(|x| x.nonce == nonce && !nonce.is_empty());
                                        drop(state);

                                        if !is_dup {
                                            let _ = local_event_tx.send(AppEvent::IncomingMessage(DiscordMessage {
                                                nonce,
                                                author: pay.d["author"]["username"].as_str().unwrap_or("Unknown").to_string(),
                                                content: pay.d["content"].as_str().unwrap_or("").to_string(),
                                                timestamp: transit_time_str,
                                                status: MessageStatus::Delivered,
                                            })).await;
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        let _ = event_tx.send(AppEvent::GatewayClosed).await;
        tokio::time::sleep(std::time::Duration::from_secs(3)).await;
    }
}
