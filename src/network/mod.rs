// src/network/mod.rs
pub mod gateway;

// FIXED: Adjusted import path layout to target the correct nested state location
use crate::app::state::AppState;
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

    // FIXED: Stripped away the old unmapped ClearOldTypers clock loop task

    // 2. Fire the secure WebSocket Listener loop
    tokio::spawn(gateway::run_gateway_loop(app_state, event_tx));
}
