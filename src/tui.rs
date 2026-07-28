use crate::app_state::{AppScreen, AppState};
use ratatui::{
    layout::{Constraint, Direction, Layout, Margin, Alignment},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph},
    Frame,
};
use terminal_pixel_animation::render_half_block;

pub fn render_ui(frame: &mut Frame, state: &AppState) {
    let size = frame.area();
    let main_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(0),
            Constraint::Length(3),
        ])
        .split(size);

    let header_text = match state.screen {
        AppScreen::Login => " Login ".to_string(),
        AppScreen::ContactList => format!(" Welcome, {}! | Contacts ", state.username),
        AppScreen::Ringing { .. } | AppScreen::Calling { .. } => " Calling... ".to_string(),
        AppScreen::InCall => {
            let mic_status = if state.is_muted { "OFF (Muted)" } else { "ON (Active)" };
            format!(" In Call | Mic: {} ", mic_status)
        },
        AppScreen::Error(_) => " Error ".to_string(),
    };

    let header = Paragraph::new(header_text)
        .style(Style::default().fg(Color::Green).add_modifier(Modifier::BOLD))
        .block(Block::default().title(" LiveKit Voice & Video TUI ").borders(Borders::ALL));
    frame.render_widget(header, main_chunks[0]);

    match &state.screen {
        AppScreen::Login => {
            let info = format!("Enter Username:\n\n> {}\u{2588}", state.input_buffer);
            let widget = Paragraph::new(info)
                .alignment(Alignment::Center)
                .block(Block::default().borders(Borders::ALL));
            frame.render_widget(widget, main_chunks[1]);
        }
        AppScreen::ContactList => {
            let mut items = Vec::new();
            for (i, user) in state.users.iter().enumerate() {
                if user == &state.username {
                    continue; // Skip self
                }
                let style = if i == state.selected_index {
                    Style::default().fg(Color::Black).bg(Color::Cyan)
                } else {
                    Style::default().fg(Color::White)
                };
                items.push(ListItem::new(format!("👤 {}", user)).style(style));
            }
            if items.is_empty() {
                items.push(ListItem::new("No other users online."));
            }
            let list = List::new(items)
                .block(Block::default().title(" Select user to call (Up/Down/Enter) ").borders(Borders::ALL));
            frame.render_widget(list, main_chunks[1]);
        }
        AppScreen::Calling { target } => {
            let info = format!("Ringing {}...\n\nWaiting for answer...", target);
            let widget = Paragraph::new(info)
                .alignment(Alignment::Center)
                .block(Block::default().borders(Borders::ALL));
            frame.render_widget(widget, main_chunks[1]);
        }
        AppScreen::Ringing { caller } => {
            let info = format!("Incoming call from {}!\n\nPress [y] to Accept or [n] to Reject.", caller);
            let widget = Paragraph::new(info)
                .alignment(Alignment::Center)
                .block(Block::default().borders(Borders::ALL));
            frame.render_widget(widget, main_chunks[1]);
        }
        AppScreen::InCall => {
            let body_chunks = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(35), Constraint::Percentage(65)])
                .split(main_chunks[1]);

            let mut participant_items = Vec::new();
            if let Some(r) = &state.livekit_room {
                participant_items.push(
                    ListItem::new(format!(
                        "{} {} (You)",
                        if state.is_muted { "🔇" } else { "🎙️" },
                        r.local_participant().identity()
                    ))
                    .style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
                );

                for (_, participant) in r.remote_participants() {
                    participant_items.push(ListItem::new(format!("🔊 {}", participant.identity())));
                }
            }

            let participants_list = List::new(participant_items).block(
                Block::default().title(" Participants ").borders(Borders::ALL),
            );
            frame.render_widget(participants_list, body_chunks[0]);

            let latest_video = {
                let lock = state.remote_video_frame.lock().unwrap();
                lock.clone()
            };

            let video_area = body_chunks[1].inner(Margin { vertical: 1, horizontal: 1 });
            let target_width = video_area.width as u32;
            let target_height = video_area.height as u32;

            if let Some((rgb, w, h)) = latest_video {
                if target_width > 0 && target_height > 0 {
                    let cells = render_half_block(&rgb, w, h, target_width, target_height).unwrap_or_default();
                    let mut lines = Vec::new();

                    for cy in 0..target_height {
                        let mut spans = Vec::new();
                        for cx in 0..target_width {
                            let idx = ((cy * target_width + cx) * 6) as usize;
                            if idx + 5 < cells.len() {
                                let r_fg = cells[idx];
                                let g_fg = cells[idx + 1];
                                let b_fg = cells[idx + 2];
                                let r_bg = cells[idx + 3];
                                let g_bg = cells[idx + 4];
                                let b_bg = cells[idx + 5];

                                spans.push(Span::styled(
                                    "\u{2580}",
                                    Style::default()
                                        .fg(Color::Rgb(r_fg, g_fg, b_fg))
                                        .bg(Color::Rgb(r_bg, g_bg, b_bg)),
                                ));
                            }
                        }
                        lines.push(Line::from(spans));
                    }

                    let video_widget = Paragraph::new(lines)
                        .block(Block::default().title(" Video Stream ").borders(Borders::ALL));
                    frame.render_widget(video_widget, body_chunks[1]);
                }
            } else {
                let info_widget = Paragraph::new("Waiting for remote video...")
                    .alignment(Alignment::Center)
                    .block(Block::default().title(" Video Stream ").borders(Borders::ALL));
                frame.render_widget(info_widget, body_chunks[1]);
            }
        }
        AppScreen::Error(msg) => {
            let info = format!("Error:\n\n{}\n\nPress any key to return.", msg);
            let widget = Paragraph::new(info)
                .style(Style::default().fg(Color::Red))
                .alignment(Alignment::Center)
                .block(Block::default().borders(Borders::ALL));
            frame.render_widget(widget, main_chunks[1]);
        }
    }

    let footer_text = match state.screen {
        AppScreen::Login => " [Enter] Submit | [q] Quit ",
        AppScreen::ContactList => " [Up/Down] Navigate | [Enter] Call | [q] Quit ",
        AppScreen::Ringing { .. } => " [y] Accept | [n] Reject | [q] Quit ",
        AppScreen::Calling { .. } => " [q] Quit ",
        AppScreen::InCall => " [m] Toggle Mute | [q] End Call ",
        AppScreen::Error(_) => " [Any Key] Dismiss ",
    };
    
    let footer = Paragraph::new(footer_text)
        .style(Style::default().fg(Color::DarkGray))
        .block(Block::default().borders(Borders::ALL));
    frame.render_widget(footer, main_chunks[2]);
}
