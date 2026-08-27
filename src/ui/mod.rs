// src/ui/mod.rs
pub mod theme;
pub mod components;
pub mod modals;

use crate::app::AppState;
use crate::models::ActivePanel;
use theme::Theme;

use ratatui::{
    layout::{Constraint, Direction, Layout},
    widgets::Clear,
    Frame,
};

pub fn render(f: &mut Frame, state: &mut AppState) {
    let total_size = f.size();
    let theme = Theme::default();

    // Split left side column from right side column
    let horizontal_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .margin(1)
        .constraints([Constraint::Percentage(25), Constraint::Percentage(75)])
        .split(total_size);

    // Split left column vertically into Server and Channel windows
    let left_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(horizontal_chunks[0]);

    // Split right column vertically into Message Feed and Text Input windows
    let right_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(3)])
        .split(horizontal_chunks[1]);

    // Assign focus styles based on active panels
    let (srv_style, chan_style, input_style) = match state.active_panel {
        ActivePanel::Servers => (theme.active_border, theme.idle_border, theme.idle_border),
        ActivePanel::Channels => (theme.idle_border, theme.active_border, theme.idle_border),
        ActivePanel::ChatInput => (theme.idle_border, theme.idle_border, theme.active_border),
        ActivePanel::LogoutPrompt => (theme.idle_border, theme.idle_border, theme.idle_border),
    };

    // Render left panel components
    let servers_widget = components::render_servers(state, srv_style);
    f.render_stateful_widget(servers_widget, left_chunks[0], &mut state.servers_state);

    let channels_widget = components::render_channels(state, chan_style);
    f.render_stateful_widget(channels_widget, left_chunks[1], &mut state.channels_state);

    // Render right feed and calculations
    let feed_height = right_chunks[0].height as usize;
    let text_rows = if feed_height > 2 { feed_height - 2 } else { 1 };
    
    let chat_widget = components::render_chat_feed(state, &theme, text_rows);
    f.render_widget(chat_widget, right_chunks[0]);

    let input_widget = components::render_input_field(state, input_style);
    f.render_widget(input_widget, right_chunks[1]);

    // Overlay Logout prompt safely over panels if activated
    if state.active_panel == ActivePanel::LogoutPrompt {
        let modal_area = modals::get_centered_bounds(50, 30, total_size);
        f.render_widget(Clear, modal_area); // Wipes out underlying layouts cleanly
        f.render_widget(modals::render_logout_prompt(), modal_area);
    }
}
