use crate::app_state::{AppScreen, AppState};
use ratatui::{
    layout::{Constraint, Direction, Layout, Margin, Alignment, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph},
    Frame,
};
use terminal_pixel_animation::{render_braille, render_half_block};

pub fn render_ui(frame: &mut Frame, state: &AppState) {
    let size = frame.area();
    frame.render_widget(Clear, size);
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
        AppScreen::Settings => format!(" Settings | {} ", state.username),
        AppScreen::Ringing { .. } | AppScreen::Calling { .. } => " Calling... ".to_string(),
        AppScreen::JoinRoom => " Join Room ".to_string(),
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
            let fields = vec![
                ("Username", &state.input_buffer, 0),
                ("LiveKit URL", &state.livekit_url, 1),
                ("API Key", &state.api_key, 2),
                ("API Secret", &state.api_secret, 3),
            ];

            let mut lines = vec![Line::from("Enter Connection Details:")];
            lines.push(Line::from(""));

            for (label, val, idx) in fields {
                let cursor = if state.active_input_index == idx { "\u{2588}" } else { "" };
                let prefix = if state.active_input_index == idx { "> " } else { "  " };
                let display_val = if idx == 3 && !val.is_empty() {
                    "*".repeat(val.len())
                } else {
                    val.clone()
                };
                
                let style = if state.active_input_index == idx {
                    Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                };

                lines.push(Line::from(Span::styled(format!("{}{}: {}{}", prefix, label, display_val, cursor), style)));
                lines.push(Line::from(""));
            }

            let widget = Paragraph::new(lines)
                .alignment(Alignment::Left)
                .block(Block::default().borders(Borders::ALL).padding(ratatui::widgets::Padding::new(4, 4, 2, 2)));
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
        AppScreen::Settings => {
            // Same layout as Login form but for editing settings after login
            let fields = vec![
                ("LiveKit URL", &state.livekit_url, 0usize),
                ("API Key",     &state.api_key,     1),
                ("API Secret",  &state.api_secret,  2),
            ];

            let renderer_val = match state.render_mode {
                crate::app_state::RenderMode::Braille => "Odin (Braille)".to_string(),
                crate::app_state::RenderMode::HalfBlock => "Zig (HalfBlock)".to_string(),
            };

            let mut lines = vec![
                Line::from(Span::styled("Connection Settings", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))),
                Line::from(""),
                Line::from(Span::styled("Changes take effect on next login.", Style::default().fg(Color::DarkGray))),
                Line::from(""),
            ];

            for (label, val, idx) in &fields {
                let cursor = if state.active_input_index == *idx { "\u{2588}" } else { "" };
                let prefix = if state.active_input_index == *idx { "> " } else { "  " };
                let display_val = if *idx == 2 && !val.is_empty() {
                    "*".repeat(val.len())
                } else {
                    (*val).clone()
                };
                let style = if state.active_input_index == *idx {
                    Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                };
                lines.push(Line::from(Span::styled(
                    format!("{}{}: {}{}", prefix, label, display_val, cursor),
                    style,
                )));
                lines.push(Line::from(""));
            }

            // Renderer field
            let cursor = if state.active_input_index == 3 { " < >" } else { "" };
            let prefix = if state.active_input_index == 3 { "> " } else { "  " };
            let style = if state.active_input_index == 3 {
                Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            lines.push(Line::from(Span::styled(
                format!("{}Renderer: {}{}", prefix, renderer_val, cursor),
                style,
            )));
            lines.push(Line::from(""));

            let widget = Paragraph::new(lines)
                .alignment(Alignment::Left)
                .block(Block::default().borders(Borders::ALL).padding(ratatui::widgets::Padding::new(4, 4, 2, 2)));
            frame.render_widget(widget, main_chunks[1]);
        }
        AppScreen::JoinRoom => {
            let mut lines = vec![
                Line::from(Span::styled("Join a room to start a group call", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))),
                Line::from(""),
                Line::from("Enter a room name. Anyone in the same room can see and hear each other."),
                Line::from(""),
            ];
            let cursor = "\u{2588}";
            let display = if state.input_buffer.is_empty() { "Type room name...".to_string() } else { state.input_buffer.clone() };
            lines.push(Line::from(Span::styled(
                format!("Room: {}{}", display, cursor),
                Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
            )));
            let widget = Paragraph::new(lines)
                .alignment(Alignment::Left)
                .block(Block::default().borders(Borders::ALL).padding(ratatui::widgets::Padding::new(4, 4, 2, 2)));
            frame.render_widget(widget, main_chunks[1]);
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

            let left_chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Min(3), Constraint::Length(7)])
                .split(body_chunks[0]);

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
            frame.render_widget(participants_list, left_chunks[0]);

            let input_level = {
                let l = state.audio_input_level.lock().unwrap();
                *l
            };
            let output_level = {
                let l = state.audio_output_level.lock().unwrap();
                *l
            };

            let meter_width = (left_chunks[1].width as usize).saturating_sub(10).min(20);
            let in_bar = render_vu_bar(input_level, meter_width);
            let out_bar = render_vu_bar(output_level, meter_width);

            let mic_status = if state.is_muted { "MUTED" } else { "ACTIVE" };
            let meter_lines = vec![
                Line::from(Span::styled(
                    format!(" IN  {} {}", in_bar, mic_status),
                    Style::default().fg(level_color(input_level)),
                )),
                Line::from(Span::styled(
                    format!(" OUT {} {}", out_bar, ""),
                    Style::default().fg(level_color(output_level)),
                )),
            ];
            let meter_widget = Paragraph::new(meter_lines)
                .block(Block::default().title(" Audio Level ").borders(Borders::ALL));
            frame.render_widget(meter_widget, left_chunks[1]);

            // Clear video area to prevent ghosting
            let video_area = body_chunks[1].inner(Margin { vertical: 1, horizontal: 1 });
            frame.render_widget(Clear, body_chunks[1]);

            let local_video = {
                let lock = state.local_video_frame.lock().unwrap();
                lock.clone()
            };
            let remote_frames = {
                let lock = state.remote_video_frames.lock().unwrap();
                lock.clone()
            };

            let mut video_parts: Vec<(String, Vec<u8>, u32, u32)> = remote_frames.into_iter().map(|(k, (rgb, w, h))| (k, rgb, w, h)).collect();
            video_parts.sort_by(|a, b| a.0.cmp(&b.0));

            if video_parts.is_empty() {
                // No remote — show self-view as a reasonable thumbnail (not fullscreen)
                if let Some((lrgb, lw, lh)) = local_video {
                    let tw = (video_area.width / 3).max(8) as u32;
                    let th = (video_area.height / 2).max(4) as u32;
                    let cx = video_area.x + video_area.width / 2 - tw as u16 / 2;
                    let cy = video_area.y + video_area.height / 2 - th as u16 / 2;
                    let rect = Rect::new(cx, cy, tw as u16, th as u16);
                    let inner_w = (tw as u16).saturating_sub(2);
                    let inner_h = (th as u16).saturating_sub(2);
                    let lines = render_video_lines(&lrgb, lw, lh, inner_w as u32, inner_h as u32, state.render_mode);
                    frame.render_widget(Clear, rect);
                    frame.render_widget(
                        Paragraph::new(lines).block(Block::default().title(" You ").borders(Borders::ALL).style(Style::default().fg(Color::Cyan))),
                        rect,
                    );
                } else {
                    frame.render_widget(
                        Paragraph::new("Waiting for remote video...")
                            .alignment(Alignment::Center)
                            .block(Block::default().title(" Video Stream ").borders(Borders::ALL)),
                        body_chunks[1],
                    );
                }
            } else {
                let n = video_parts.len();
                let (cols, rows) = grid_dims(n);
                let tile_w = (video_area.width as u32 / cols).max(6);
                let tile_h = (video_area.height as u32 / rows).max(4);
                // Content area inside borders
                let inner_w = (tile_w as u16).saturating_sub(2);
                let inner_h = (tile_h as u16).saturating_sub(2);

                for (i, (identity, rgb, w, h)) in video_parts.iter().enumerate() {
                    let col = i as u32 % cols;
                    let row = i as u32 / cols;
                    let x = video_area.x + (col * tile_w) as u16;
                    let y = video_area.y + (row * tile_h) as u16;
                    let tile_rect = Rect::new(x, y, tile_w as u16, tile_h as u16);

                    if tile_rect.right() > video_area.right() || tile_rect.bottom() > video_area.bottom() {
                        continue;
                    }

                    let lines = render_video_lines(rgb, *w, *h, inner_w as u32, inner_h as u32, state.render_mode);
                    frame.render_widget(Clear, tile_rect);
                    frame.render_widget(
                        Paragraph::new(lines)
                            .block(Block::default().title(format!(" {} ", identity)).borders(Borders::ALL).style(Style::default().fg(Color::Green))),
                        tile_rect,
                    );
                }

                // Self-view PiP in top-right corner
                if let Some((lrgb, lw, lh)) = local_video {
                    let pip_w = (tile_w / 3).max(4) as u16;
                    let pip_h = (tile_h / 3).max(2) as u16;
                    let inner_pip_w = pip_w.saturating_sub(2);
                    let inner_pip_h = pip_h.saturating_sub(2);
                    let pip_x = video_area.right().saturating_sub(pip_w).saturating_sub(1);
                    let pip_y = video_area.y + 1;
                    let pip_area = Rect::new(pip_x, pip_y, pip_w, pip_h);
                    if pip_area.right() <= video_area.right() && pip_area.bottom() <= video_area.bottom() {
                        let pip_lines = render_video_lines(&lrgb, lw, lh, inner_pip_w as u32, inner_pip_h as u32, state.render_mode);
                        frame.render_widget(Clear, pip_area);
                        frame.render_widget(
                            Paragraph::new(pip_lines)
                                .block(Block::default().title(" You ").borders(Borders::ALL).style(Style::default().fg(Color::Cyan))),
                            pip_area,
                        );
                    }
                }
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
        AppScreen::Login    => " [Tab] Next Field | [Enter] Connect | [Esc] Quit ",
        AppScreen::ContactList => " [Up/Down] Navigate | [Enter] Call | [j] Join Room | [s] Settings | [q] Quit ",
        AppScreen::Settings => " [Tab] Next Field | [Enter] Save & Back | [Esc] Cancel ",
        AppScreen::JoinRoom => " [Enter] Join Room | [Esc] Back ",
        AppScreen::Ringing { .. } => " [y] Accept | [n] Reject | [q] Quit ",
        AppScreen::Calling { .. } => " [q] Quit ",
        AppScreen::InCall   => " [m] Toggle Mic | [r] Toggle Renderer | [q] End Call ",
        AppScreen::Error(_) => " [Any Key] Dismiss ",
    };
    
    let footer = Paragraph::new(footer_text)
        .style(Style::default().fg(Color::DarkGray))
        .block(Block::default().borders(Borders::ALL));
    frame.render_widget(footer, main_chunks[2]);
}

fn grid_dims(n: usize) -> (u32, u32) {
    if n == 0 { return (1, 1); }
    if n <= 1 { return (1, 1); }
    if n <= 2 { return (2, 1); }
    if n <= 4 { return (2, 2); }
    if n <= 6 { return (3, 2); }
    if n <= 9 { return (3, 3); }
    let cols = (n as f64).sqrt().ceil() as u32;
    let rows = ((n as f64) / cols as f64).ceil() as u32;
    (cols, rows)
}

fn render_video_lines(rgb: &[u8], w: u32, h: u32, target_w: u32, target_h: u32, mode: crate::app_state::RenderMode) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    match mode {
        crate::app_state::RenderMode::Braille => {
            let cells = render_braille(rgb, w, h, target_w, target_h).unwrap_or_default();
            for cy in 0..target_h {
                let mut spans = Vec::new();
                for cx in 0..target_w {
                    let idx = ((cy * target_w + cx) * 8) as usize;
                    if idx + 7 < cells.len() {
                        let code_point = u32::from_le_bytes([
                            cells[idx], cells[idx + 1], cells[idx + 2], cells[idx + 3],
                        ]);
                        let ch = char::from_u32(code_point).unwrap_or(' ');
                        let r = cells[idx + 4];
                        let g = cells[idx + 5];
                        let b = cells[idx + 6];
                        let s = if ch == '\0' || ch == ' ' { " ".to_string() } else { ch.to_string() };
                        spans.push(Span::styled(s, Style::default().fg(Color::Rgb(r, g, b))));
                    }
                }
                lines.push(Line::from(spans));
            }
        }
        crate::app_state::RenderMode::HalfBlock => {
            let cells = render_half_block(rgb, w, h, target_w, target_h).unwrap_or_default();
            for cy in 0..target_h {
                let mut spans = Vec::new();
                for cx in 0..target_w {
                    let idx = ((cy * target_w + cx) * 6) as usize;
                    if idx + 5 < cells.len() {
                        spans.push(Span::styled(
                            "\u{2580}",
                            Style::default()
                                .fg(Color::Rgb(cells[idx], cells[idx + 1], cells[idx + 2]))
                                .bg(Color::Rgb(cells[idx + 3], cells[idx + 4], cells[idx + 5])),
                        ));
                    }
                }
                lines.push(Line::from(spans));
            }
        }
    }
    lines
}

fn render_vu_bar(level: f32, width: usize) -> String {
    if width == 0 {
        return String::new();
    }
    let filled = ((level * 8.0).min(1.0).sqrt() * width as f32) as usize;
    let filled = filled.min(width);
    let empty = width.saturating_sub(filled);
    let bar: String = std::iter::repeat('█').take(filled)
        .chain(std::iter::repeat('░').take(empty))
        .collect();
    format!("[{}]", bar)
}

fn level_color(level: f32) -> Color {
    if level > 0.5 {
        Color::Red
    } else if level > 0.2 {
        Color::Yellow
    } else if level > 0.01 {
        Color::Green
    } else {
        Color::DarkGray
    }
}
