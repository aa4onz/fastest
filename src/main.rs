// src/main.rs
pub mod models;
pub mod network;
pub mod app;

use models::AppEvent;
use std::io::{self, Write};
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut token = String::new();
    let mut url_input = String::new();

    // 1. Read Cached or New Personal User Token
    if std::path::Path::new(".token_cache").exists() {
        token = std::fs::read_to_string(".token_cache")?.trim().to_string();
    } else {
        print!("Enter your Discord Personal User Token: ");
        io::stdout().flush()?;
        io::stdin().read_line(&mut token)?;
        token = token.trim().to_string();
        std::fs::write(".token_cache", &token)?;
    }

    // 2. Read Target Discord Channel URL
    print!("Enter direct Discord Channel URL link: ");
    io::stdout().flush()?;
    io::stdin().read_line(&mut url_input)?;
    url_input = url_input.trim().to_string();

    // 3. Dynamic URL Splitter: Extract Channel ID from the URL format
    let target_channel_id = url_input
        .split('/')
        .last()
        .unwrap_or("")
        .to_string();

    if target_channel_id.is_empty() || !target_channel_id.chars().all(|c| c.is_numeric()) {
        println!("Error: Invalid Discord Channel URL provided!");
        return Ok(());
    }

    // 4. Initialize Lightweight Core TUI Terminal Window Environment
    crossterm::terminal::enable_raw_mode()?;
    let mut stdout = io::stdout();
    crossterm::queue!(stdout, crossterm::terminal::EnterAlternateScreen, crossterm::cursor::Hide)?;
    let backend = ratatui::backend::CrosstermBackend::new(stdout);
    let mut terminal = ratatui::Terminal::new(backend)?;

    // 5. Instantiate Global Memory Application States
    let mut initial_state = crate::app::state::AppState::new(token.clone());
    initial_state.target_channel_id = target_channel_id.clone();

    let app_state = Arc::new(Mutex::new(initial_state));

    // 6. Setup Asynchronous Message Pipeline Channels
    let (event_tx, mut event_rx) = mpsc::channel::<AppEvent>(100);

    // 7. Instantiate Bare-bones Global HTTP Pool Client with Browser Identity Mocking
    let http_client = reqwest::Client::builder()
        .tcp_nodelay(true)
        .pool_max_idle_per_host(10)
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
        .build()
        .unwrap();

    // 8. Spawn High-Performance Network Gateways
    network::spawn_network_handlers(Arc::clone(&app_state), event_tx.clone(), http_client.clone());

    // 9. Primary Application Render & Key Polling Loop
    loop {
        let app_state_clone = Arc::clone(&app_state);
        terminal.draw(|f| {
            let size = f.size();
            let chunks = ratatui::layout::Layout::default()
                .direction(ratatui::layout::Direction::Vertical)
                .constraints([
                    ratatui::layout::Constraint::Min(3),
                    ratatui::layout::Constraint::Length(3),
                ])
                .split(size);

            if let Ok(state) = app_state_clone.try_lock() {
                let msgs: Vec<ratatui::widgets::ListItem> = state.messages.iter().map(|m| {
                    let status_indicator = match m.status {
                        models::MessageStatus::Sending => " [...]",
                        models::MessageStatus::Failed => " [❌]",
                        models::MessageStatus::Delivered => "",
                    };
                    ratatui::widgets::ListItem::new(format!(
                        "[{}] <{}>: {}{}",
                        m.timestamp, m.author, m.content, status_indicator
                    ))
                }).collect();

                let msg_list = ratatui::widgets::List::new(msgs)
                    .block(ratatui::widgets::Block::default()
                        .borders(ratatui::widgets::Borders::ALL)
                        .title(format!(" Locked Channel ID: {} ", state.target_channel_id)));
                
                f.render_widget(msg_list, chunks[0]);

                let input_box = ratatui::widgets::Paragraph::new(state.input_text.as_str())
                    .block(ratatui::widgets::Block::default()
                        .borders(ratatui::widgets::Borders::ALL)
                        .title(" Type Chat Message (Press Enter to Send) "));
                
                f.render_widget(input_box, chunks[1]);
            }
        })?;

        // Process Incoming System Thread Events
        if let Some(event) = event_rx.recv().await {
            let mut state = app_state.lock().await;
            let should_exit = state.handle_event(event, &event_tx, &http_client).await;
            if should_exit { break; }
        }
    }

    // 10. Graceful Restoration of Local Machine Operating System Terminal Mode
    crossterm::terminal::disable_raw_mode()?;
    crossterm::execute!(
        terminal.backend_mut(),
        crossterm::terminal::LeaveAlternateScreen,
        crossterm::cursor::Show
    )?;
    Ok(())
}
