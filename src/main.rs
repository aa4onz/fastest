// src/main.rs - PART 1
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

    // Load or request the Discord user token safely from local cache file systems
    if std::path::Path::new(".token_cache").exists() {
        token = std::fs::read_to_string(".token_cache")?.trim().to_string();
    } else {
        print!("Enter your Discord Personal User Token: ");
        io::stdout().flush()?;
        io::stdin().read_line(&mut token)?;
        token = token.trim().to_string();
        std::fs::write(".token_cache", &token)?;
    }

    print!("Enter direct Discord Channel URL link: ");
    io::stdout().flush()?;
    io::stdin().read_line(&mut url_input)?;
    url_input = url_input.trim().to_string();

    let target_channel_id = url_input.split('/').last().unwrap_or("").to_string();
    if target_channel_id.is_empty() || !target_channel_id.chars().all(|c| c.is_numeric()) {
        println!("Error: Invalid Discord Channel URL provided!");
        return Ok(());
    }

    // Initialize raw terminal modes and enter Crossterm alternative window view buffers
    crossterm::terminal::enable_raw_mode()?;
    let mut stdout = io::stdout();
    crossterm::queue!(stdout, crossterm::terminal::EnterAlternateScreen, crossterm::cursor::Hide)?;
    let backend = ratatui::backend::CrosstermBackend::new(stdout);
    let mut terminal = ratatui::Terminal::new(backend)?;

    let mut initial_state = crate::app::state::AppState::new(token.clone());
    initial_state.target_channel_id = target_channel_id.clone();
    let app_state = Arc::new(Mutex::new(initial_state));

    // Core communication channels: event_rx handles ui, net_rx passes to your network worker module
    let (event_tx, mut event_rx) = mpsc::channel::<AppEvent>(100);
    let (net_tx, net_rx) = mpsc::channel::<AppEvent>(50); 
    
    // ⚡ SPOOFED HTTP CLIENT FOR BYPASSING DISCORD AUTOMATED SECURITY FILTERS
    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert("accept", "*/*".parse().unwrap());
    headers.insert("accept-language", "en-US,en;q=0.9".parse().unwrap());
    headers.insert("sec-ch-ua", "\"Chromium\";v=\"128\", \"Not;A=Brand\";v=\"24\", \"Google Chrome\";v=\"128\"".parse().unwrap());
    headers.insert("sec-ch-ua-mobile", "?0".parse().unwrap());
    headers.insert("sec-ch-ua-platform", "\"Windows\"".parse().unwrap());
    headers.insert("sec-fetch-dest", "empty".parse().unwrap());
    headers.insert("sec-fetch-mode", "cors".parse().unwrap());
    headers.insert("sec-fetch-site", "same-origin".parse().unwrap());
    headers.insert("x-debug-options", "bugReporterEnabled".parse().unwrap());
    headers.insert("x-discord-timezone", "America/New_York".parse().unwrap());
    
    // This base64 payload mimics an official secure Windows 10 desktop client profile layer
    headers.insert("x-super-properties", "eyJvcyI6IldpbmRvd3MiLCJicm93c2VyIjoiQ2hyb21lIiwiZGV2aWNlIjoiIiwicmVmZXJyZXIiOiJodHRwczovL2Rpc2NvcmQuY29tLyIsIm9zX3ZlcnNpb24iOiIxMCIsImJyb3dzZXJfdmVyc2lvbiI6IjEyOC4wLjAuMCIsImJsdWV0b290aF9lbmFibGVkIjpmYWxzZX0=".parse().unwrap());

    // Persistent HTTP pool connection engine optimization settings with embedded browser profiles
    let http_client = reqwest::Client::builder()
        .tcp_nodelay(true)
        .pool_max_idle_per_host(5)
        .pool_idle_timeout(std::time::Duration::from_secs(120))
        .default_headers(headers)
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/128.0.0.0 Safari/537.36")
        .build()
        .unwrap();

    // Boots the background handlers inside the network folder ecosystem ecosystem
    network::spawn_network_handlers(Arc::clone(&app_state), event_tx.clone(), http_client.clone(), net_rx);
// src/main.rs - PART 2
    // Draw the interface layout exactly once when the program boots up
    let app_state_clone = Arc::clone(&app_state);
    {
        let mut state = app_state_clone.lock().await;
        let len = state.messages.len();
        if len > 0 {
            state.list_state.select(Some(len - 1));
        }
    }
    
    terminal.draw(|f| {
        let screen_size = f.size();
        let horizontal_chunks = ratatui::layout::Layout::default()
            .direction(ratatui::layout::Direction::Horizontal)
            .constraints([
                ratatui::layout::Constraint::Percentage(15),
                ratatui::layout::Constraint::Percentage(70),
                ratatui::layout::Constraint::Percentage(15),
            ])
            .split(screen_size);

        let middle_area = horizontal_chunks[1];
        let vertical_chunks = ratatui::layout::Layout::default()
            .direction(ratatui::layout::Direction::Vertical)
            .constraints([
                ratatui::layout::Constraint::Min(3),
                ratatui::layout::Constraint::Length(3), 
            ])
            .split(middle_area);

        if let Ok(mut state) = app_state_clone.try_lock() {
            let msg_list = ratatui::widgets::List::new(Vec::<ratatui::widgets::ListItem>::new())
                .block(ratatui::widgets::Block::default().borders(ratatui::widgets::Borders::ALL).title(" messages "));
            f.render_stateful_widget(msg_list, vertical_chunks[0], &mut state.list_state);

            let input_box = ratatui::widgets::Paragraph::new("> ")
                .block(ratatui::widgets::Block::default().borders(ratatui::widgets::Borders::ALL));
            f.render_widget(input_box, vertical_chunks[1]);
        }
    })?;

    // ⚡ THE EVENT PIPELINE: Process actions instantly, then draw exactly once
    while let Some(event) = event_rx.recv().await {
        match event {
            // Instantly offload intensive outbound HTTP network actions down the channel pipeline
            AppEvent::HttpTriggerTyping | AppEvent::HttpSendChat { .. } => {
                let _ = net_tx.send(event).await;
            }
            _ => {
                let mut state = app_state.lock().await;
                let should_exit = state.handle_event(event, &event_tx).await;
                if should_exit { break; }
            }
        }

        // 🌟 THUT REPAINT SEQUENCE: Redraws view states only when updates occur
        let app_state_clone = Arc::clone(&app_state);
        
        {
            let mut state = app_state_clone.lock().await;
            let len = state.messages.len();
            if len > 0 {
                state.list_state.select(Some(len - 1));
            }
        }

        terminal.draw(|f| {
            let screen_size = f.size();
            
            let horizontal_chunks = ratatui::layout::Layout::default()
                .direction(ratatui::layout::Direction::Horizontal)
                .constraints([
                    ratatui::layout::Constraint::Percentage(15),
                    ratatui::layout::Constraint::Percentage(70),
                    ratatui::layout::Constraint::Percentage(15),
                ])
                .split(screen_size);

            let middle_area = horizontal_chunks[1];

            let vertical_chunks = ratatui::layout::Layout::default()
                .direction(ratatui::layout::Direction::Vertical)
                .constraints([
                    ratatui::layout::Constraint::Min(3),
                    ratatui::layout::Constraint::Length(3), 
                ])
                .split(middle_area);

            if let Ok(mut state) = app_state_clone.try_lock() {
                let msgs: Vec<ratatui::widgets::ListItem> = state.messages.iter().map(|m| {
                    use ratatui::style::{Color, Style};
                    use ratatui::text::{Line, Span};

                    let is_me = m.author == "You";
                    let author_color = if is_me { Color::Blue } else { Color::Green };
                    let header_style = Style::default().fg(author_color);

                    let content_color = match m.status {
                        models::MessageStatus::Sending => Color::DarkGray,
                        models::MessageStatus::Failed => Color::Red,
                        models::MessageStatus::Delivered => Color::White,
                    };

                    let status_indicator = match m.status {
                        models::MessageStatus::Sending => " [...]",
                        models::MessageStatus::Failed => " [❌]",
                        models::MessageStatus::Delivered => "",
                    };

                    let header_line = Line::from(vec![
                        Span::styled(format!("{}", m.author), header_style),
                        Span::raw(" "),
                        Span::styled(format!("[{} {}]", m.timestamp, status_indicator), header_style),
                    ]);

                    let content_line = Line::from(vec![
                        Span::styled(format!("  {}", m.content), Style::default().fg(content_color))
                    ]);

                    ratatui::widgets::ListItem::new(vec![header_line, content_line])
                }).collect();

                let msg_list = ratatui::widgets::List::new(msgs)
                    .block(ratatui::widgets::Block::default()
                        .borders(ratatui::widgets::Borders::ALL)
                        .title(" messages "));
                
                f.render_stateful_widget(msg_list, vertical_chunks[0], &mut state.list_state);

                let prompt_span = ratatui::text::Span::raw("> ");
                let text_span = ratatui::text::Span::styled(
                    state.input_text.as_str(),
                    ratatui::style::Style::default().fg(ratatui::style::Color::Yellow)
                );
                let input_line = ratatui::text::Line::from(vec![prompt_span, text_span]);

                let input_box = ratatui::widgets::Paragraph::new(input_line)
                    .block(ratatui::widgets::Block::default()
                        .borders(ratatui::widgets::Borders::ALL));
                
                f.render_widget(input_box, vertical_chunks[1]);
            }
        })?;
    }

    // Clean terminal workspace state rollback upon user exiting application loop environments
    crossterm::terminal::disable_raw_mode()?;
    crossterm::execute!(terminal.backend_mut(), crossterm::terminal::LeaveAlternateScreen, crossterm::cursor::Show)?;
    Ok(())
}
