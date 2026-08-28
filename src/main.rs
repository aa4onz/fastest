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

    crossterm::terminal::enable_raw_mode()?;
    let mut stdout = io::stdout();
    crossterm::queue!(stdout, crossterm::terminal::EnterAlternateScreen, crossterm::cursor::Hide)?;
    let backend = ratatui::backend::CrosstermBackend::new(stdout);
    let mut terminal = ratatui::Terminal::new(backend)?;

    let mut initial_state = crate::app::state::AppState::new(token.clone());
    initial_state.target_channel_id = target_channel_id.clone();
    let app_state = Arc::new(Mutex::new(initial_state));

    let (event_tx, mut event_rx) = mpsc::channel::<AppEvent>(100);
    let http_client = reqwest::Client::builder()
        .tcp_nodelay(true)
        .pool_max_idle_per_host(10)
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36")
        .build()
        .unwrap();

    network::spawn_network_handlers(Arc::clone(&app_state), event_tx.clone(), http_client.clone());

    loop {
        let app_state_clone = Arc::clone(&app_state);
        
        {
            let mut state = app_state_clone.lock().await;
            let len = state.messages.len();
            if len > 0 {
                state.list_state.select(Some(len - 1));
            }
        }

        terminal.draw(|f| {
            let size = f.size();
            
            // 🟢 FIXED CONSTRAINT REGIONS: Split layout cleanly with explicit border boundaries
            let chunks = ratatui::layout::Layout::default()
                .direction(ratatui::layout::Direction::Vertical)
                .constraints([
                    ratatui::layout::Constraint::Min(3),    // Pane A: Chat feed logs window
                    ratatui::layout::Constraint::Length(3),  // Pane B: Heavy Bordered input box container field
                ])
                .split(size);

            if let Ok(mut state) = app_state_clone.try_lock() {
                // 1. Map messages list items with color highlights
                let msgs: Vec<ratatui::widgets::ListItem> = state.messages.iter().map(|m| {
                    use ratatui::style::{Color, Style};
                    use ratatui::text::{Line, Span};

                    let is_me = m.author == "You";
                    let author_color = if is_me { Color::Blue } else { Color::Green };
                    let header_style = Style::default().fg(author_color);
                    let time_style = Style::default().fg(Color::DarkGray);

                    let status_indicator = match m.status {
                        models::MessageStatus::Sending => " [...]",
                        models::MessageStatus::Failed => " [❌]",
                        models::MessageStatus::Delivered => "",
                    };

                    let header_line = Line::from(vec![
                        Span::styled(format!("{}", m.author), header_style),
                        Span::raw(" "),
                        Span::styled(format!("[{} {}]", m.timestamp, status_indicator), time_style),
                    ]);

                    let content_line = Line::from(vec![
                        Span::raw(format!("  {}", m.content))
                    ]);

                    ratatui::widgets::ListItem::new(vec![header_line, content_line])
                }).collect();

                // 🟢 ADDED BORDERS: Created a solid border wrapper frame around the chat viewport area
                let msg_list = ratatui::widgets::List::new(msgs)
                    .block(ratatui::widgets::Block::default()
                        .borders(ratatui::widgets::Borders::ALL)
                        .title(format!(" Locked Channel ID: {} ", state.target_channel_id)));
                
                f.render_stateful_widget(msg_list, chunks[0], &mut state.list_state);

                // 2. Compute dynamic multi-user typing status strings
                let current_time = std::time::Instant::now();
                let typing_names: Vec<String> = state.typing_users
                    .iter()
                    .filter_map(|(name, inst)| {
                        if current_time.duration_since(*inst).as_secs() < 6 && name != "You" {
                            Some(name.clone())
                        } else {
                            None
                        }
                    })
                    .collect();

                // 🟢 TYPING EMBED POSITION: Create a compact tracking flag header block
                let typing_prefix = if typing_names.is_empty() {
                    "".to_string()
                } else if typing_names.len() == 1 {
                    format!("(💬 {} typing...) ", typing_names[0])
                } else {
                    "(💬 Users typing...) ".to_string()
                };

                // 🟢 OVERLAP PREVENTION: Merge typing indicator and prompt directly side-by-side
                let input_str = format!("{}> {}", typing_prefix, state.input_text);
                
                // 🟢 ADDED BORDERS: Creates a matching dedicated structural boundary pane around text entry lanes
                let input_box = ratatui::widgets::Paragraph::new(input_str)
                    .block(ratatui::widgets::Block::default()
                        .borders(ratatui::widgets::Borders::ALL)
                        .title(" Type Chat Message (Press Enter to Send) "));
                
                f.render_widget(input_box, chunks[1]);
            }
        })?;

        if let Some(event) = event_rx.recv().await {
            let mut state = app_state.lock().await;
            let should_exit = state.handle_event(event, &event_tx, &http_client).await;
            if should_exit { break; }
        }
    }

    crossterm::terminal::enable_raw_mode()?;
    crossterm::execute!(terminal.backend_mut(), crossterm::terminal::LeaveAlternateScreen, crossterm::cursor::Show)?;
    Ok(())
}
