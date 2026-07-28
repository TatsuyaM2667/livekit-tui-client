use livekit::prelude::Room;
use ratatui::{
    layout::{Constraint, Direction, Layout, Margin},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph},
    Frame,
};
use std::sync::{Arc, Mutex};
use terminal_pixel_animation::render_half_block;

pub fn render_ui(
    frame: &mut Frame,
    room: Option<&Room>,
    is_muted: bool,
    error_msg: &str,
    remote_video_frame: &Arc<Mutex<Option<(Vec<u8>, u32, u32)>>>,
) {
    let size = frame.area();
    let main_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(0),
            Constraint::Length(3),
        ])
        .split(size);

    let header_text = if room.is_some() {
        let mic_status = if is_muted { "OFF (Muted)" } else { "ON (Active)" };
        format!(
            " Room: test-room  |  Status: Connected  |  Mic: {}",
            mic_status
        )
    } else {
        format!(" Status: Disconnected ({})", error_msg)
    };

    let header = Paragraph::new(header_text)
        .style(
            Style::default()
                .fg(if is_muted { Color::Yellow } else { Color::Green })
                .add_modifier(Modifier::BOLD),
        )
        .block(
            Block::default()
                .title(" LiveKit Voice & Video TUI ")
                .borders(Borders::ALL),
        );
    frame.render_widget(header, main_chunks[0]);

    let body_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(35), Constraint::Percentage(65)])
        .split(main_chunks[1]);

    let mut participant_items = Vec::new();
    if let Some(r) = room {
        participant_items.push(
            ListItem::new(format!(
                "{} {} (You)",
                if is_muted { "🔇" } else { "🎙️" },
                r.local_participant().identity()
            ))
            .style(
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
        );

        for (_, participant) in r.remote_participants() {
            participant_items.push(ListItem::new(format!("🔊 {}", participant.identity())));
        }
    }

    let participants_list = List::new(participant_items).block(
        Block::default()
            .title(" Participants ")
            .borders(Borders::ALL),
    );
    frame.render_widget(participants_list, body_chunks[0]);

    // Attempt to grab the latest video frame
    let latest_video = {
        let lock = remote_video_frame.lock().unwrap();
        lock.clone()
    };

    let video_area = body_chunks[1].inner(Margin {
        vertical: 1,
        horizontal: 1,
    });
    let target_width = video_area.width as u32;
    let target_height = video_area.height as u32;

    if let Some((rgb, w, h)) = latest_video {
        if target_width > 0 && target_height > 0 {
            // Render using terminal-pixel-animation
            let cells =
                render_half_block(&rgb, w, h, target_width, target_height).unwrap_or_default();
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
        let info_text = if room.is_some() {
            format!(
                "LiveKit Voice & Camera Streaming Active!\n\n\
                 - [m] : Toggle Mute\n\
                 - [q] : Quit\n\n\
                 Users in Room: {}\n\n\
                 (Waiting for remote video...)",
                room.map(|r| r.remote_participants().len() + 1).unwrap_or(0)
            )
        } else {
            "Disconnected".to_string()
        };

        let info_widget = Paragraph::new(info_text)
            .block(Block::default().title(" Status ").borders(Borders::ALL));
        frame.render_widget(info_widget, body_chunks[1]);
    }

    let footer = Paragraph::new(" [m] Toggle Mute  |  [q] Quit App")
        .style(Style::default().fg(Color::DarkGray))
        .block(Block::default().borders(Borders::ALL));
    frame.render_widget(footer, main_chunks[2]);
}
