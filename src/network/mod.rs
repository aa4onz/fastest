// src/network/mod.rs
pub mod gateway;

use crate::app::AppState;
use crate::models::AppEvent;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc::Sender, Mutex};

pub fn spawn_network_handlers(app_state: Arc<Mutex<AppState>>, event_tx: Sender<AppEvent>, _http_client: reqwest::Client) {
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

    // 2. Typing UI expiration garbage-collector loop
    let janitor_tx = event_tx.clone();
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_secs(1)).await;
            let _ = janitor_tx.send(AppEvent::ClearOldTypers).await;
        }
    });

    // 3. Fire the secure WebSocket Listener loop (Handles 100% of data loading now)
    tokio::spawn(gateway::run_gateway_loop(app_state, event_tx));
}
