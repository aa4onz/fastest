// src/network/mod.rs
pub mod gateway;
pub mod http;

use crate::app::state::AppState;
use crate::models::AppEvent;
use crate::network::http::DiscordHttpClient;
use std::sync::Arc;
use tokio::sync::{mpsc, mpsc::Sender, Mutex};

pub fn spawn_network_handlers(
    app_state: Arc<Mutex<AppState>>, 
    event_tx: Sender<AppEvent>, 
    http_client: reqwest::Client,
    net_rx: mpsc::Receiver<AppEvent>,
) {
    // Get token and channel ID safely out of current state structure initialization
    let (token, target_channel_id) = {
        let state = app_state.try_lock().expect("Failed to lock state at initialization");
        (state.token.clone(), state.target_channel_id.clone())
    };

    let client_wrapper = DiscordHttpClient::new(http_client.clone(), token);

    // 1. ⚡ ULTRA-FAST INSTANT CROSSTERM KEY CAPTURE (NO POLLING TIMEOUT LAG)
    let input_tx = event_tx.clone();
    tokio::task::spawn_blocking(move || {
        loop {
            // Block on raw OS key reads. Executes the exact microsecond your finger registers input.
            if let Ok(ev) = crossterm::event::read() {
                if let Err(_) = input_tx.blocking_send(AppEvent::Terminal(ev)) {
                    break; // Event loop shut down, exit thread
                }
            }
        }
    });

    // 2. Fire the secure WebSocket Listener loop
    tokio::spawn(gateway::run_gateway_loop(Arc::clone(&app_state), event_tx.clone()));

    // 3. ⚡ EXCLUSIVE ASYNC OUTBOUND HTTP WORKER
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
