// src/network/mod.rs
pub mod gateway;
pub mod http;

use crate::app::state::AppState;
use crate::models::AppEvent;
use crate::network::http::DiscordHttpClient;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, mpsc::Sender, Mutex};

pub fn spawn_network_handlers(
    app_state: Arc<Mutex<AppState>>, 
    event_tx: Sender<AppEvent>, 
    http_client: reqwest::Client,
    net_rx: mpsc::Receiver<AppEvent>,
) {
    // Get token and channel ID safely out of current state structure initialization
    let (token, target_channel_id) = {
        // Block to unlock state instantly
        let state = app_state.try_lock().expect("Failed to lock state at initialization");
        (state.token.clone(), state.target_channel_id.clone())
    };

    let client_wrapper = DiscordHttpClient::new(http_client.clone(), token);

    // 1. Asynchronously poll Crossterm terminal input key events
    let input_tx = event_tx.clone();
    tokio::spawn(async move {
        loop {
            if let Ok(true) = crossterm::event::poll(Duration::from_millis(50)) {
                if let Ok(ev) = crossterm::event::read() {
                    let _ = input_tx.send(AppEvent::Terminal(ev)).await;
                }
            }
        }
    });

    // 2. Fire the secure WebSocket Listener loop
    tokio::spawn(gateway::run_gateway_loop(Arc::clone(&app_state), event_tx.clone()));

    // 3. ⚡ EXCLUSIVE ASYNC OUTBOUND HTTP WORKER
    // Runs inside the network folder ecosystem. Listens to net_rx queue.
    let worker_tx = event_tx.clone();
    let mut outbound_rx = net_rx;
    
    tokio::spawn(async move {
        while let Some(job) = outbound_rx.recv().await {
            match job {
                AppEvent::HttpTriggerTyping => {
                    let _ = client_wrapper.send_typing(&target_channel_id).await;
                }
                AppEvent::HttpSendChat { nonce, text } => {
                    let res = client_wrapper.send_message(&target_channel_id, &text, &nonce).await;

                    if let Ok(resp) = res {
                        if resp.status().is_success() {
                            let _ = worker_tx.send(AppEvent::MessageSent {
                                nonce,
                                timestamp: String::new(),
                            }).await;
                        } else {
                            let raw_err = resp.text().await.unwrap_or_else(|_| "Rejected".to_string());
                            let parsed_err = if let Some(idx) = raw_err.find("message\":") {
                                raw_err[idx + 9..].chars().filter(|c| *c != '"' && *c != '}' && *c != '{').take(80).collect::<String>()
                            } else {
                                raw_err.chars().filter(|c| !c.is_control() && *c != '{' && *c != '}' && *c != '"').take(80).collect::<String>()
                            };
                            let combined_err_string = format!(
                                "{} | ❌ {}",
                                chrono::Local::now().format("%H:%M:%S"),
                                parsed_err.trim()
                            );

                            let _ = worker_tx.send(AppEvent::MessageSent {
                                nonce: nonce.clone(),
                                timestamp: combined_err_string,
                            }).await;
                            let _ = worker_tx.send(AppEvent::MessageFailed { nonce }).await;
                        }
                    } else {
                        let _ = worker_tx.send(AppEvent::MessageFailed { nonce }).await;
                    }
                }
                _ => {}
            }
        }
    });
}
