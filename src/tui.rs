use crate::app_state::{AppScreen, AppState, StatusKind};
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Margin, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph},
    Frame,
};
use terminal_pixel_animation::{render_braille, render_half_block};

pub fn render_ui(frame: &mut Frame, state: &AppState) {
    let size = frame.area();
    // 全画面クリアで前画面の残骸を必ず消去
    frame.render_widget(Clear, size);

    let main_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(0),
            Constraint::Length(3),
        ])
        .split(size);

    let header_text = match &state.screen {
        AppScreen::Login => " Login ".to_string(),
        AppScreen::ContactList => format!(" Welcome, {}! | Contacts ", state.username),
        AppScreen::Settings => format!(" Settings | {} ", state.username),
        AppScreen::Ringing { .. } | AppScreen::Calling { .. } => " Calling... ".to_string(),
        AppScreen::JoinRoom => " Group Call — Create or Join a Room ".to_string(),
        AppScreen::InCall => {
            let mic_status = if state.is_muted {
                "OFF (Muted)"
            } else {
                "ON (Active)"
            };
            let room_info = if state.room_name.is_empty() {
                String::new()
            } else {
                format!(" | Room: {}", state.room_name)
            };
            format!(" In Call | Mic: {}{} ", mic_status, room_info)
        }
        AppScreen::Error(_) => " Error ".to_string(),
        AppScreen::RoomBrowser => " Public Room Browser ".to_string(),
        AppScreen::InviteRoom { room_name, .. } => format!(" Invite to \"{}\" ", room_name),
    };

    let header = Paragraph::new(header_text)
        .style(
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        )
        .block(
            Block::default()
                .title(" LiveKit Voice & Video TUI ")
                .borders(Borders::ALL),
        );
    frame.render_widget(header, main_chunks[0]);

    match &state.screen {
        AppScreen::Login => {
            render_login(frame, state, main_chunks[1]);
        }
        AppScreen::ContactList => {
            render_contact_list(frame, state, main_chunks[1]);
        }
        AppScreen::Settings => {
            render_settings(frame, state, main_chunks[1]);
        }
        AppScreen::JoinRoom => {
            render_join_room(frame, state, main_chunks[1]);
        }
        AppScreen::Calling { target } => {
            let info = format!("{}に発信中...\n\n応答を待っています...", target);
            let widget = Paragraph::new(info)
                .alignment(Alignment::Center)
                .block(Block::default().borders(Borders::ALL));
            frame.render_widget(widget, main_chunks[1]);
        }
        AppScreen::Ringing { caller } => {
            let info = format!("{}から着信中\n\n[y] 応答  [n] 拒否", caller);
            let widget = Paragraph::new(info)
                .alignment(Alignment::Center)
                .block(Block::default().borders(Borders::ALL));
            frame.render_widget(widget, main_chunks[1]);
        }
        AppScreen::InCall => {
            render_in_call(frame, state, main_chunks[1]);
        }
        AppScreen::RoomBrowser => {
            render_room_browser(frame, state, main_chunks[1]);
        }
        AppScreen::InviteRoom {
            room_name,
            invited_users,
        } => {
            render_invite_room(frame, state, main_chunks[1], room_name, invited_users);
        }
        AppScreen::Error(msg) => {
            let info = format!("エラー:\n\n{}\n\n任意のキーで戻る", msg);
            let widget = Paragraph::new(info)
                .style(Style::default().fg(Color::Red))
                .alignment(Alignment::Center)
                .block(Block::default().borders(Borders::ALL));
            frame.render_widget(widget, main_chunks[1]);
        }
    }

    let footer_text = match &state.screen {
        AppScreen::Login => " [Tab] 次のフィールド | [Enter] 接続 | [Esc] 終了 ",
        AppScreen::ContactList => {
            " [↑↓] 移動 | [Enter] 発信 | [j] ルーム作成 | [b] ルームブラウザ | [s] 設定 | [q] 終了 "
        }
        AppScreen::Settings => " [Tab] 次のフィールド | [Enter] 保存して戻る | [Esc] キャンセル ",
        AppScreen::JoinRoom => " [Enter] 招待画面へ | [p] 公開/非公開切替 | [Esc] 戻る ",
        AppScreen::RoomBrowser => {
            " [↑↓] 移動 | [Enter] 参加 | [c] コンタクトリスト | [j] ルーム作成 | [Esc] 戻る "
        }
        AppScreen::InviteRoom { .. } => {
            " [↑↓] 移動 | [Space] 選択/解除 | [Enter] 招待して参加 | [Esc] 戻る "
        }
        AppScreen::Ringing { .. } => " [y] 応答 | [n] 拒否 | [q] 終了 ",
        AppScreen::Calling { .. } => " [q/Esc] キャンセル ",
        AppScreen::InCall => " [m] マイク | [r] レンダラ切替 | [q/Esc] 通話終了 ",
        AppScreen::Error(_) => " [任意のキー] 閉じる ",
    };

    let footer = Paragraph::new(footer_text)
        .style(Style::default().fg(Color::DarkGray))
        .block(Block::default().borders(Borders::ALL));
    frame.render_widget(footer, main_chunks[2]);
}

// ── ログイン画面 ──────────────────────────────────────────────────────────────

fn render_login(frame: &mut Frame, state: &AppState, area: Rect) {
    let fields = vec![
        ("ユーザー名", &state.input_buffer, 0),
        ("LiveKit URL", &state.livekit_url, 1),
        ("API Key", &state.api_key, 2),
        ("API Secret", &state.api_secret, 3),
    ];

    let mut lines = vec![Line::from(Span::styled(
        "接続情報を入力...",
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD),
    ))];
    lines.push(Line::from(""));

    for (label, val, idx) in fields {
        let cursor = if state.active_input_index == idx {
            "\u{2588}"
        } else {
            ""
        };
        let prefix = if state.active_input_index == idx {
            "> "
        } else {
            "  "
        };
        let display_val = if idx == 3 && !val.is_empty() {
            "*".repeat(val.len())
        } else {
            val.clone()
        };

        let style = if state.active_input_index == idx {
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default()
        };

        lines.push(Line::from(Span::styled(
            format!("{}{}: {}{}", prefix, label, display_val, cursor),
            style,
        )));
        lines.push(Line::from(""));
    }

    let widget = Paragraph::new(lines).alignment(Alignment::Left).block(
        Block::default()
            .borders(Borders::ALL)
            .padding(ratatui::widgets::Padding::new(4, 4, 2, 2)),
    );
    frame.render_widget(widget, area);
}

// ── コンタクトリスト ──────────────────────────────────────────────────────────

fn render_contact_list(frame: &mut Frame, state: &AppState, area: Rect) {
    let quality_map = {
        let q = state.participant_quality.lock().unwrap();
        q.clone()
    };

    let mut items = Vec::new();
    let filtered: Vec<&String> = state
        .users
        .iter()
        .filter(|u| *u != &state.username)
        .collect();

    for (i, user) in filtered.iter().enumerate() {
        let sig = quality_map.get(*user).copied().unwrap_or(0);
        let icon = quality_icon(sig);
        let bars = signal_bars_detail(sig);

        let style = if i == state.selected_index {
            Style::default().fg(Color::Black).bg(Color::Cyan)
        } else {
            Style::default().fg(Color::White)
        };
        items.push(ListItem::new(format!("👤 {}  {} {}", user, icon, bars)).style(style));
    }

    if items.is_empty() {
        items.push(ListItem::new(Span::styled(
            "  オンラインのユーザーはいません",
            Style::default().fg(Color::DarkGray),
        )));
    }

    let list = List::new(items).block(
        Block::default()
            .title(" ユーザーを選んで発信 (↑↓ Enter) ")
            .borders(Borders::ALL),
    );
    frame.render_widget(list, area);
}

// ── 設定画面 ─────────────────────────────────────────────────────────────────

fn render_settings(frame: &mut Frame, state: &AppState, area: Rect) {
    let fields: Vec<(&str, &String, usize)> = vec![
        ("LiveKit URL", &state.livekit_url, 0usize),
        ("API Key", &state.api_key, 1),
        ("API Secret", &state.api_secret, 2),
    ];

    let renderer_val = match state.render_mode {
        crate::app_state::RenderMode::Braille => "Odin (Braille)".to_string(),
        crate::app_state::RenderMode::HalfBlock => "Zig (HalfBlock)".to_string(),
    };

    let mut lines = vec![
        Line::from(Span::styled(
            "接続設定",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "次回ログイン時に反映されます",
            Style::default().fg(Color::DarkGray),
        )),
        Line::from(""),
    ];

    for (label, val, idx) in &fields {
        let cursor = if state.active_input_index == *idx {
            "\u{2588}"
        } else {
            ""
        };
        let prefix = if state.active_input_index == *idx {
            "> "
        } else {
            "  "
        };
        let display_val = if *idx == 2 && !val.is_empty() {
            "*".repeat(val.len())
        } else {
            (*val).clone()
        };
        let style = if state.active_input_index == *idx {
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default()
        };
        lines.push(Line::from(Span::styled(
            format!("{}{}: {}{}", prefix, label, display_val, cursor),
            style,
        )));
        lines.push(Line::from(""));
    }

    let cursor = if state.active_input_index == 3 {
        " < >"
    } else {
        ""
    };
    let prefix = if state.active_input_index == 3 {
        "> "
    } else {
        "  "
    };
    let style = if state.active_input_index == 3 {
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default()
    };
    lines.push(Line::from(Span::styled(
        format!("{}レンダラ: {}{}", prefix, renderer_val, cursor),
        style,
    )));
    lines.push(Line::from(""));

    let widget = Paragraph::new(lines).alignment(Alignment::Left).block(
        Block::default()
            .borders(Borders::ALL)
            .padding(ratatui::widgets::Padding::new(4, 4, 2, 2)),
    );
    frame.render_widget(widget, area);
}

// ── JoinRoom 画面 ─────────────────────────────────────────────────────────────

fn render_join_room(frame: &mut Frame, state: &AppState, area: Rect) {
    // 左: 入力フォーム / 右: オンラインユーザー一覧
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
        .split(area);

    let visibility_str = if state.room_is_public {
        "公開 (Public)"
    } else {
        "非公開 (Private)"
    };
    let visibility_color = if state.room_is_public {
        Color::Green
    } else {
        Color::Red
    };

    let mut lines = vec![
        Line::from(Span::styled(
            "グループ通話 — ルームを作成/参加",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "同じルーム名のユーザー同士で通話できます",
            Style::default().fg(Color::DarkGray),
        )),
        Line::from(""),
        Line::from(vec![
            Span::styled("公開設定: ", Style::default().fg(Color::White)),
            Span::styled(
                visibility_str,
                Style::default()
                    .fg(visibility_color)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled("  — [p] で切替", Style::default().fg(Color::DarkGray)),
        ]),
        Line::from(""),
        Line::from(Span::styled("ルーム名:", Style::default().fg(Color::White))),
        Line::from(""),
    ];

    let cursor = "\u{2588}";
    let display = if state.input_buffer.is_empty() {
        "例: team-meeting-2024".to_string()
    } else {
        state.input_buffer.clone()
    };
    let style = if state.input_buffer.is_empty() {
        Style::default().fg(Color::DarkGray)
    } else {
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD)
    };
    lines.push(Line::from(Span::styled(
        format!("  {}{}", display, cursor),
        style,
    )));
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "  [Enter] 招待画面へ  |  [p] 公開/非公開  |  [Esc] 戻る",
        Style::default().fg(Color::DarkGray),
    )));
    lines.push(Line::from(""));
    if state.room_is_public {
        lines.push(Line::from(Span::styled(
            "公開ルームはルームブラウザに表示されます",
            Style::default().fg(Color::Green),
        )));
    }

    let widget = Paragraph::new(lines).alignment(Alignment::Left).block(
        Block::default()
            .borders(Borders::ALL)
            .padding(ratatui::widgets::Padding::new(2, 2, 1, 1)),
    );
    frame.render_widget(widget, chunks[0]);

    // 右パネル: 招待可能なオンラインユーザー
    let online: Vec<&String> = state
        .users
        .iter()
        .filter(|u| *u != &state.username)
        .collect();
    let mut user_items: Vec<ListItem> = online
        .iter()
        .map(|u| ListItem::new(format!("👤 {}", u)).style(Style::default().fg(Color::White)))
        .collect();
    if user_items.is_empty() {
        user_items.push(ListItem::new(Span::styled(
            "  オンラインユーザーなし",
            Style::default().fg(Color::DarkGray),
        )));
    }
    let user_list = List::new(user_items).block(
        Block::default()
            .title(" オンラインユーザー ")
            .borders(Borders::ALL),
    );
    frame.render_widget(user_list, chunks[1]);
}

// ── InCall 画面 ──────────────────────────────────────────────────────────────

fn render_in_call(frame: &mut Frame, state: &AppState, area: Rect) {
    let body_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(35), Constraint::Percentage(65)])
        .split(area);

    let left_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(3),
            Constraint::Length(6),
            Constraint::Length(7),
        ])
        .split(body_chunks[0]);

    let quality_map = {
        let q = state.participant_quality.lock().unwrap();
        q.clone()
    };

    // ── 参加者リスト ──
    let mut participant_items = Vec::new();
    if let Some(r) = &state.livekit_room {
        participant_items.push(
            ListItem::new(format!(
                "{} {} (You)",
                if state.is_muted { "🔇" } else { "🎙️" },
                r.local_participant().identity()
            ))
            .style(
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
        );

        for (_, participant) in r.remote_participants() {
            let id = participant.identity().as_str().to_string();
            let sig = quality_map.get(&id).copied().unwrap_or(0);
            let icon = quality_icon(sig);
            let bars = signal_bars_detail(sig);
            let sig_label = match sig {
                0 => "接続中",
                1 => "弱",
                2 => "良好",
                3 => "強",
                _ => "?",
            };
            participant_items.push(ListItem::new(Line::from(vec![
                Span::styled("🎙️ ", Style::default()),
                Span::styled(id.clone(), Style::default().fg(Color::White)),
                Span::raw("  "),
                Span::styled(
                    format!("{} {} ({})", icon, bars, sig_label),
                    Style::default().fg(signal_color(sig)),
                ),
            ])));
        }
    }

    let participants_list = List::new(participant_items)
        .block(Block::default().title(" 参加者 ").borders(Borders::ALL));
    frame.render_widget(participants_list, left_chunks[0]);

    // ── ステータスメッセージ ──
    let status_msgs = {
        let m = state.status_messages.lock().unwrap();
        m.clone()
    };
    let disconnected = {
        let d = state.disconnected_peer.lock().unwrap();
        d.clone()
    };

    let msg_lines: Vec<Line> = status_msgs
        .iter()
        .rev()
        .take(4)
        .map(|(m, kind)| {
            let color = match kind {
                StatusKind::Join => Color::Green,
                StatusKind::Leave => Color::Red,
                StatusKind::Info => Color::Yellow,
            };
            Line::from(Span::styled(m.clone(), Style::default().fg(color)))
        })
        .collect();

    let notif_title = if disconnected.is_some() {
        " ⚠ 通知 (切断あり) "
    } else {
        " 通知 "
    };
    let notif_border_style = if disconnected.is_some() {
        Style::default().fg(Color::Red)
    } else {
        Style::default()
    };
    let msg_widget = Paragraph::new(msg_lines).block(
        Block::default()
            .title(notif_title)
            .borders(Borders::ALL)
            .style(notif_border_style),
    );
    frame.render_widget(msg_widget, left_chunks[1]);

    // ── 音声レベルメーター ──
    let input_level = {
        let l = state.audio_input_level.lock().unwrap();
        *l
    };
    let output_level = {
        let l = state.audio_output_level.lock().unwrap();
        *l
    };

    let meter_width = (left_chunks[2].width as usize).saturating_sub(10).min(20);
    let in_bar = render_vu_bar(input_level, meter_width);
    let out_bar = render_vu_bar(output_level, meter_width);

    let mic_status = if state.is_muted { "MUTED" } else { "ACTIVE" };
    let meter_lines = vec![
        Line::from(Span::styled(
            format!(" IN  {} {}", in_bar, mic_status),
            Style::default().fg(level_color(input_level)),
        )),
        Line::from(Span::styled(
            format!(" OUT {} ", out_bar),
            Style::default().fg(level_color(output_level)),
        )),
    ];
    let meter_widget = Paragraph::new(meter_lines)
        .block(Block::default().title(" 音声レベル ").borders(Borders::ALL));
    frame.render_widget(meter_widget, left_chunks[2]);

    // ── 動画エリア ──
    frame.render_widget(Clear, body_chunks[1]);
    let video_area = body_chunks[1].inner(Margin {
        vertical: 1,
        horizontal: 1,
    });

    let local_video = {
        let lock = state.local_video_frame.lock().unwrap();
        lock.clone()
    };
    let remote_frames = {
        let lock = state.remote_video_frames.lock().unwrap();
        lock.clone()
    };

    let mut video_parts: Vec<(String, Vec<u8>, u32, u32)> = remote_frames
        .into_iter()
        .map(|(k, (rgb, w, h))| (k, rgb, w, h))
        .collect();
    video_parts.sort_by(|a, b| a.0.cmp(&b.0));

    if video_parts.is_empty() {
        if let Some((lrgb, lw, lh)) = local_video {
            let tw = (video_area.width / 3).max(8) as u32;
            let th = (video_area.height / 2).max(4) as u32;
            let cx = video_area.x + video_area.width / 2 - tw as u16 / 2;
            let cy = video_area.y + video_area.height / 2 - th as u16 / 2;
            let rect = Rect::new(cx, cy, tw as u16, th as u16);
            let inner_w = (tw as u16).saturating_sub(2);
            let inner_h = (th as u16).saturating_sub(2);
            let lines = render_video_lines(
                &lrgb,
                lw,
                lh,
                inner_w as u32,
                inner_h as u32,
                state.render_mode,
            );
            frame.render_widget(Clear, rect);
            frame.render_widget(
                Paragraph::new(lines).block(
                    Block::default()
                        .title(" You ")
                        .borders(Borders::ALL)
                        .style(Style::default().fg(Color::Cyan)),
                ),
                rect,
            );
        } else {
            frame.render_widget(
                Paragraph::new("リモート映像を待っています...")
                    .alignment(Alignment::Center)
                    .block(
                        Block::default()
                            .title(" Video Stream ")
                            .borders(Borders::ALL),
                    ),
                body_chunks[1],
            );
        }
    } else {
        let n = video_parts.len();
        let (cols, rows) = grid_dims(n);
        let tile_w = (video_area.width as u32 / cols).max(6);
        let tile_h = (video_area.height as u32 / rows).max(4);
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

            let lines = render_video_lines(
                rgb,
                *w,
                *h,
                inner_w as u32,
                inner_h as u32,
                state.render_mode,
            );
            frame.render_widget(Clear, tile_rect);
            frame.render_widget(
                Paragraph::new(lines).block(
                    Block::default()
                        .title(format!(" {} ", identity))
                        .borders(Borders::ALL)
                        .style(Style::default().fg(Color::Green)),
                ),
                tile_rect,
            );
        }

        // Self-view PiP
        if let Some((lrgb, lw, lh)) = local_video {
            let pip_w = (tile_w / 3).max(4) as u16;
            let pip_h = (tile_h / 3).max(2) as u16;
            let inner_pip_w = pip_w.saturating_sub(2);
            let inner_pip_h = pip_h.saturating_sub(2);
            let pip_x = video_area.right().saturating_sub(pip_w).saturating_sub(1);
            let pip_y = video_area.y + 1;
            let pip_area = Rect::new(pip_x, pip_y, pip_w, pip_h);
            if pip_area.right() <= video_area.right() && pip_area.bottom() <= video_area.bottom() {
                let pip_lines = render_video_lines(
                    &lrgb,
                    lw,
                    lh,
                    inner_pip_w as u32,
                    inner_pip_h as u32,
                    state.render_mode,
                );
                frame.render_widget(Clear, pip_area);
                frame.render_widget(
                    Paragraph::new(pip_lines).block(
                        Block::default()
                            .title(" You ")
                            .borders(Borders::ALL)
                            .style(Style::default().fg(Color::Cyan)),
                    ),
                    pip_area,
                );
            }
        }
    }

    // ── 切断ポップアップオーバーレイ ──
    if let Some(peer_name) = {
        let d = state.disconnected_peer.lock().unwrap();
        d.clone()
    } {
        let popup_w = 40u16.min(size_clamp(area.width, 40));
        let popup_h = 5u16;
        let popup_x = area.x + (area.width.saturating_sub(popup_w)) / 2;
        let popup_y = area.y + (area.height.saturating_sub(popup_h)) / 2;
        let popup_area = Rect::new(popup_x, popup_y, popup_w, popup_h);
        frame.render_widget(Clear, popup_area);
        let popup_text = format!("⚠  {} が切断しました\n\n[任意のキー] で閉じる", peer_name);
        frame.render_widget(
            Paragraph::new(popup_text)
                .alignment(Alignment::Center)
                .style(Style::default().fg(Color::Red))
                .block(
                    Block::default()
                        .title(" 切断通知 ")
                        .borders(Borders::ALL)
                        .style(Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)),
                ),
            popup_area,
        );
    }
}

fn size_clamp(total: u16, desired: u16) -> u16 {
    desired.min(total)
}

// ── RoomBrowser 画面 ──────────────────────────────────────────────────────────

fn render_room_browser(frame: &mut Frame, state: &AppState, area: Rect) {
    let announced = {
        let a = state.announced_rooms.lock().unwrap();
        a.clone()
    };

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(3)])
        .split(area);

    if announced.is_empty() {
        let lines = vec![
            Line::from(Span::styled(
                "公開ルームブラウザ",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
            Line::from(Span::styled(
                "現在参加可能な公開ルームはありません",
                Style::default().fg(Color::DarkGray),
            )),
            Line::from(""),
            Line::from(Span::styled(
                "  [j] グループ通話ルームを作成して公開",
                Style::default().fg(Color::Green),
            )),
            Line::from(""),
            Line::from(Span::styled(
                "  [c] 個人通話リスト",
                Style::default().fg(Color::DarkGray),
            )),
        ];
        let widget = Paragraph::new(lines).alignment(Alignment::Left).block(
            Block::default()
                .borders(Borders::ALL)
                .padding(ratatui::widgets::Padding::new(4, 4, 2, 2)),
        );
        frame.render_widget(widget, area);
        return;
    }

    let mut items: Vec<ListItem> = Vec::new();
    for (i, room) in announced.iter().enumerate() {
        let style = if i == state.selected_index {
            Style::default().fg(Color::Black).bg(Color::Cyan)
        } else {
            Style::default().fg(Color::White)
        };
        let marker = if i == state.selected_index {
            "▶ "
        } else {
            "  "
        };
        items.push(ListItem::new(Line::from(vec![
            Span::styled(marker, style),
            Span::styled(
                format!("🏠 {} ", room.name),
                style.add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("(作成者: {})", room.owner),
                Style::default().fg(Color::DarkGray),
            ),
        ])));
    }

    let list = List::new(items).block(
        Block::default()
            .title(" 参加可能な公開ルーム一覧 ")
            .borders(Borders::ALL),
    );
    frame.render_widget(list, chunks[0]);

    let hint =
        Paragraph::new(" [↑↓] 移動  [Enter] 参加  [c] コンタクト  [j] ルーム作成  [Esc] 戻る ")
            .style(Style::default().fg(Color::DarkGray))
            .block(Block::default().borders(Borders::ALL));
    frame.render_widget(hint, chunks[1]);
}

// ── InviteRoom 画面 ──────────────────────────────────────────────────────────

fn render_invite_room(
    frame: &mut Frame,
    state: &AppState,
    area: Rect,
    room_name: &str,
    invited_users: &[String],
) {
    let filtered: Vec<&String> = state
        .users
        .iter()
        .filter(|u| *u != &state.username)
        .collect();

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(5), Constraint::Min(0)])
        .split(area);

    // ヘッダー説明
    let header_lines = vec![
        Line::from(Span::styled(
            format!("ルーム \"{}\" に招待するユーザーを選択", room_name),
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "[Space] で選択/解除  [Enter] で招待して参加  [Esc] で戻る",
            Style::default().fg(Color::DarkGray),
        )),
    ];
    let header_widget = Paragraph::new(header_lines).block(
        Block::default()
            .borders(Borders::ALL)
            .padding(ratatui::widgets::Padding::new(2, 2, 0, 0)),
    );
    frame.render_widget(header_widget, chunks[0]);

    // ユーザーリスト
    let mut items: Vec<ListItem> = Vec::new();
    if filtered.is_empty() {
        items.push(ListItem::new(Span::styled(
            "  招待可能なオンラインユーザーなし",
            Style::default().fg(Color::DarkGray),
        )));
    } else {
        for (i, user) in filtered.iter().enumerate() {
            let is_selected = invited_users.contains(user);
            let is_cursor = i == state.selected_index;
            let checkbox = if is_selected { "[✓]" } else { "[ ]" };

            let (bg, fg) = if is_cursor {
                (Color::DarkGray, Color::White)
            } else {
                (Color::Reset, Color::White)
            };
            let check_color = if is_selected {
                Color::Green
            } else {
                Color::DarkGray
            };

            items.push(ListItem::new(Line::from(vec![
                Span::styled(
                    if is_cursor { "▶ " } else { "  " },
                    Style::default().fg(Color::Cyan),
                ),
                Span::styled(
                    checkbox,
                    Style::default()
                        .fg(check_color)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(format!(" 👤 {}", user), Style::default().fg(fg).bg(bg)),
            ])));
        }
    }

    let selected_count = invited_users.len();
    let title = format!(" ユーザーを招待 ({} 人選択中) ", selected_count);
    let list = List::new(items).block(Block::default().title(title.as_str()).borders(Borders::ALL));
    frame.render_widget(list, chunks[1]);
}

// ── ユーティリティ ────────────────────────────────────────────────────────────

fn grid_dims(n: usize) -> (u32, u32) {
    if n == 0 {
        return (1, 1);
    }
    if n <= 1 {
        return (1, 1);
    }
    if n <= 2 {
        return (2, 1);
    }
    if n <= 4 {
        return (2, 2);
    }
    if n <= 6 {
        return (3, 2);
    }
    if n <= 9 {
        return (3, 3);
    }
    let cols = (n as f64).sqrt().ceil() as u32;
    let rows = ((n as f64) / cols as f64).ceil() as u32;
    (cols, rows)
}

fn render_video_lines(
    rgb: &[u8],
    w: u32,
    h: u32,
    target_w: u32,
    target_h: u32,
    mode: crate::app_state::RenderMode,
) -> Vec<Line<'static>> {
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
                            cells[idx],
                            cells[idx + 1],
                            cells[idx + 2],
                            cells[idx + 3],
                        ]);
                        let ch = char::from_u32(code_point).unwrap_or(' ');
                        let r = cells[idx + 4];
                        let g = cells[idx + 5];
                        let b = cells[idx + 6];
                        let s = if ch == '\0' || ch == ' ' {
                            " ".to_string()
                        } else {
                            ch.to_string()
                        };
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
    let bar: String = std::iter::repeat('█')
        .take(filled)
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

fn quality_icon(q: u8) -> &'static str {
    match q {
        0 => "⏳",
        1 => "🔴",
        2 => "🟡",
        3 => "🟢",
        _ => "❓",
    }
}

/// 電波強度を縦バーグラフで表現（詳細版）
fn signal_bars_detail(q: u8) -> &'static str {
    match q {
        0 => "▁▁▁", // 接続中/不明
        1 => "▂▁▁", // 弱
        2 => "▄▄▁", // 良好
        3 => "█▆▄", // 強
        _ => "   ",
    }
}

fn signal_color(q: u8) -> Color {
    match q {
        0 => Color::DarkGray,
        1 => Color::Red,
        2 => Color::Yellow,
        3 => Color::Green,
        _ => Color::DarkGray,
    }
}
