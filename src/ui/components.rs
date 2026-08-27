// src/ui/components.rs
use crate::app::AppState;
use crate::models::MessageStatus;
use crate::ui::theme::Theme;
use ratatui::{
    style::{Modifier, Style, Color},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph},
};

pub fn render_servers(state: &AppState, style: Style) -> List<'static> {
    let items: Vec<ListItem> = state.servers.iter().enumerate().map(|(i, s)| {
        let prefix = if i == state.selected_server_idx { ">> " } else { "   " };
        ListItem::new(format!("{}{}", prefix, s.name))
    }).collect();

    List::new(items)
        .block(Block::default().title(" servers ").borders(Borders::ALL).border_style(style))
}

pub fn render_channels(state: &AppState, style: Style) -> List<'static> {
    let current_channels = &state.servers[state.selected_server_idx].channels;
    let items: Vec<ListItem> = current_channels.iter().enumerate().map(|(i, c)| {
        let prefix = if i == state.selected_channel_idx { ">> " } else { "   " };
        ListItem::new(format!("{}# {}", prefix, c.name))
    }).collect();

    List::new(items)
        .block(Block::default().title(" channels ").borders(Borders::ALL).border_style(style))
}

pub fn render_chat_feed(state: &AppState, theme: &Theme, available_rows: usize) -> Paragraph<'static> {
    let mut chat_lines = Vec::new();

    for msg in state.messages.iter() {
        match msg.status {
            MessageStatus::Sending => {
                chat_lines.push(Line::from(vec![
                    Span::styled(msg.author.clone(), theme.self_message),
                    Span::styled(format!(" [{}]", msg.timestamp), theme.system_text),
                ]));
                chat_lines.push(Line::from(vec![Span::styled(format!("  {}", msg.content), theme.system_text)]));
            }
            MessageStatus::Delivered => {
                let author_style = if msg.author == "You" { theme.self_message } else { theme.peer_message };
                chat_lines.push(Line::from(vec![
                    Span::styled(msg.author.clone(), author_style),
                    Span::styled(format!(" [{}]", msg.timestamp), Style::default().fg(Color::Gray)),
                ]));
                chat_lines.push(Line::from(vec![Span::styled(format!("  {}", msg.content), Style::default().fg(Color::White))]));
            }
            MessageStatus::Failed => {
                chat_lines.push(Line::from(vec![
                    Span::styled(msg.author.clone(), theme.error_text),
                    Span::styled(format!(" [{}]", msg.timestamp), theme.error_text),
                ]));
                chat_lines.push(Line::from(vec![
                    Span::styled(format!("  {}", msg.content), theme.error_text.add_modifier(Modifier::CROSSED_OUT)),
                ]));
            }
        }
    }

    // Handle typing notifications footer text
    let footer_text = match state.typing_users.get(&state.current_channel_id()) {
        Some(typers) if !typers.is_empty() => {
            let names: Vec<String> = typers.keys().cloned().collect();
            if names.len() == 1 {
                format!(" ✍️ {} is typing... ", names[0])
            } else {
                " ✍️ Several people are typing... ".to_string()
            }
        }
        _ => format!(" channel: #{} ", state.current_channel_name()),
    };

    let total_lines = chat_lines.len();
    let visible_lines = if total_lines > available_rows {
        chat_lines.into_iter().skip(total_lines - available_rows).collect()
    } else {
        chat_lines
    };

    Paragraph::new(visible_lines).block(Block::default().title(footer_text).borders(Borders::ALL))
}

pub fn render_input_field(state: &AppState, style: Style) -> Paragraph<'static> {
    Paragraph::new(format!("> {}", state.input_text))
        .block(Block::default().title(" chat context [Ctrl+X to Logout] ").borders(Borders::ALL).border_style(style))
}
