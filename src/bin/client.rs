use anyhow::Result;
use crossterm::{
    event::{self, Event, KeyCode},
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    ExecutableCommand,
};
use jsonwebtoken::{encode, Algorithm, EncodingKey, Header};
use livekit::prelude::*;
use livekit_tui_client::{
    app_state::{AppScreen, AppState},
    audio, config, events,
    shared::SignalingMessage,
    tui, video,
};
use ratatui::{backend::CrosstermBackend, Terminal};
use serde::{Deserialize, Serialize};
use std::io::stdout;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::mpsc;

// ── Token generation ──────────────────────────────────────────────────────────

#[derive(Serialize, Deserialize)]
#[allow(non_snake_case)]
struct VideoGrants {
    roomCreate: bool,
    roomJoin: bool,
    room: String,
}

#[derive(Serialize, Deserialize)]
struct Claims {
    iss: String,
    sub: String,
    name: String,
    video: VideoGrants,
    exp: usize,
    nbf: usize,
}

fn create_token(api_key: &str, api_secret: &str, identity: &str, room: &str) -> Result<String> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)?
        .as_secs() as usize;

    let claims = Claims {
        iss: api_key.to_string(),
        sub: identity.to_string(),
        name: identity.to_string(),
        video: VideoGrants {
            roomCreate: true,
            roomJoin: true,
            room: room.to_string(),
        },
        exp: now + 3600,
        nbf: now,
    };

    let mut header = Header::new(Algorithm::HS256);
    header.typ = Some("JWT".to_string());

    Ok(encode(
        &header,
        &claims,
        &EncodingKey::from_secret(api_secret.as_bytes()),
    )?)
}

// ── Data Channel helpers ──────────────────────────────────────────────────────

/// Send a SignalingMessage to a specific participant via LiveKit Data Channel.
/// We broadcast to all (reliable) and filter on the receiver side by `to` field.
async fn send_signaling(room: &Room, msg: &SignalingMessage) -> Result<()> {
    let json = serde_json::to_string(msg)?;
    room.local_participant()
        .publish_data(
            DataPacket {
                payload: json.into_bytes(),
                reliable: true,
                ..Default::default()
            },
        )
        .await?;
    Ok(())
}

// ── Main ──────────────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();

    enable_raw_mode()?;
    stdout().execute(EnterAlternateScreen)?;
    stdout().execute(crossterm::event::EnableBracketedPaste)?;
    let mut terminal = Terminal::new(CrosstermBackend::new(stdout()))?;

    let mut state = AppState::new();
    let mut audio_pub: Option<LocalTrackPublication> = None;

    audio::diagnose_audio();

    // Channel for signaling messages received from Data Channel
    let (tx_sig, mut rx_sig) = mpsc::unbounded_channel::<SignalingMessage>();

    // Shared state for lobby room events (participant list updates)
    let participant_list: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));

    loop {
        terminal.draw(|frame| {
            tui::render_ui(frame, &state);
        })?;

        // ── Login screen: enter username and join lobby ───────────────────────
        if state.screen == AppScreen::Login {
            if event::poll(Duration::from_millis(50))? {
                match event::read()? {
                    Event::Key(key) => {
                        match key.code {
                            KeyCode::Tab | KeyCode::Down => {
                                state.active_input_index = (state.active_input_index + 1) % 4;
                            }
                            KeyCode::BackTab | KeyCode::Up => {
                                state.active_input_index = (state.active_input_index + 3) % 4;
                            }
                            KeyCode::Enter => {
                                let username = state.input_buffer.trim().to_string();
                                if !username.is_empty() {
                                    match create_token(&state.api_key, &state.api_secret, &username, "lobby") {
                                        Ok(token) => {
                                            match Room::connect(&state.livekit_url, &token, RoomOptions::default()).await {
                                                Ok((lobby, rx_lobby)) => {
                                                    state.username = username.clone();
                                                    state.input_buffer.clear();

                                                    let mode_str = match state.render_mode {
                                                        livekit_tui_client::app_state::RenderMode::Braille => "braille".to_string(),
                                                        livekit_tui_client::app_state::RenderMode::HalfBlock => "halfblock".to_string(),
                                                    };

                                                    // Save config on successful login
                                                    let _ = config::save(&livekit_tui_client::config::Config {
                                                        livekit_url: state.livekit_url.clone(),
                                                        api_key: state.api_key.clone(),
                                                        api_secret: state.api_secret.clone(),
                                                        last_username: username.clone(),
                                                        render_mode: Some(mode_str),
                                                    });

                                                    // Collect current participants
                                                    {
                                                        let mut list = participant_list.lock().unwrap();
                                                        *list = lobby
                                                            .remote_participants()
                                                            .keys()
                                                            .map(|id| id.as_str().to_string())
                                                            .collect();
                                                    }
                                                    state.users = participant_list.lock().unwrap().clone();

                                                    state.livekit_lobby = Some(lobby);
                                                    state.screen = AppScreen::ContactList;

                                                    // Spawn lobby event handler
                                                    let tx_sig_clone = tx_sig.clone();
                                                    let pl_clone = participant_list.clone();
                                                    let my_name = username.clone();
                                                    tokio::spawn(async move {
                                                        handle_lobby_events(rx_lobby, tx_sig_clone, pl_clone, my_name).await;
                                                    });
                                                }
                                                Err(e) => {
                                                    state.screen = AppScreen::Error(
                                                        format!("Failed to connect to LiveKit: {}", e),
                                                    );
                                                }
                                            }
                                        }
                                        Err(e) => {
                                            state.screen = AppScreen::Error(format!("Token error: {}", e));
                                        }
                                    }
                                }
                            }
                            KeyCode::Char(c) => {
                                match state.active_input_index {
                                    0 => state.input_buffer.push(c),
                                    1 => state.livekit_url.push(c),
                                    2 => state.api_key.push(c),
                                    3 => state.api_secret.push(c),
                                    _ => {}
                                }
                            }
                            KeyCode::Backspace => {
                                match state.active_input_index {
                                    0 => { state.input_buffer.pop(); }
                                    1 => { state.livekit_url.pop(); }
                                    2 => { state.api_key.pop(); }
                                    3 => { state.api_secret.pop(); }
                                    _ => {}
                                }
                            }
                            KeyCode::Esc => break,
                            _ => {}
                        }
                    }
                    Event::Paste(text) => {
                        // strip newlines to prevent weird cursor breaks in text inputs
                        let safe_text = text.replace('\n', "").replace('\r', "");
                        match state.active_input_index {
                            0 => state.input_buffer.push_str(&safe_text),
                            1 => state.livekit_url.push_str(&safe_text),
                            2 => state.api_key.push_str(&safe_text),
                            3 => state.api_secret.push_str(&safe_text),
                            _ => {}
                        }
                    }
                    _ => {}
                }
            }
            continue;
        }

        // ── Handle incoming signaling messages from lobby Data Channel ─────────
        while let Ok(msg) = rx_sig.try_recv() {
            match &msg {
                SignalingMessage::CallRequest { from, to } if to == &state.username => {
                    if state.screen == AppScreen::ContactList {
                        state.screen = AppScreen::Ringing { caller: from.clone() };
                    } else {
                        // Busy — auto-reject
                        if let Some(lobby) = &state.livekit_lobby {
                            let reject = SignalingMessage::CallRejected {
                                from: state.username.clone(),
                                to: from.clone(),
                            };
                            let _ = send_signaling(lobby, &reject).await;
                        }
                    }
                }
                SignalingMessage::CallAccepted { from, to, room } if to == &state.username => {
                    // We are the caller — join the call room
                    let room_name = room.clone();
                    let callee = from.clone();
                    match create_token(&state.api_key, &state.api_secret, &state.username, &room_name) {
                        Ok(token) => {
                            match Room::connect(&state.livekit_url, &token, RoomOptions::default()).await {
                                Ok((call_room, rx_call)) => {
                                    state.screen = AppScreen::InCall;
                                    audio_pub = audio::setup_microphone(&call_room, state.audio_input_level.clone()).await;
                                    video::setup_camera(&call_room, state.local_video_frame.clone()).await;
                                    events::handle_room_events(rx_call, state.remote_video_frames.clone(), state.audio_output_level.clone());
                                    state.livekit_room = Some(call_room);
                                }
                                Err(e) => {
                                    state.screen = AppScreen::Error(format!(
                                        "Failed to join call room with {}: {}", callee, e
                                    ));
                                }
                            }
                        }
                        Err(e) => {
                            state.screen = AppScreen::Error(format!("Token error: {}", e));
                        }
                    }
                }
                SignalingMessage::CallRejected { from, to } if to == &state.username => {
                    if let AppScreen::Calling { .. } = &state.screen {
                        state.screen = AppScreen::Error(format!("{} rejected the call.", from));
                    }
                }
                SignalingMessage::CallEnded { from, .. } => {
                    if let AppScreen::InCall = &state.screen {
                        if let Some(r) = state.livekit_room.take() {
                            let _ = r.close().await;
                        }
                        state.screen = AppScreen::ContactList;
                    }
                    let _ = from; // suppress unused warning
                }
                _ => {} // Message not addressed to us
            }
        }

        // ── Update participant list from shared state ──────────────────────────
        {
            let list = participant_list.lock().unwrap().clone();
            state.users = list;
            if state.selected_index >= state.users.len() && !state.users.is_empty() {
                state.selected_index = state.users.len() - 1;
            }
        }

        // ── Handle keyboard input ─────────────────────────────────────────────
        if event::poll(Duration::from_millis(50))? {
            match event::read()? {
                Event::Key(key) => {
                    match &state.screen {
                        AppScreen::ContactList => {
                            let filtered: Vec<String> = state
                                .users
                                .iter()
                                .filter(|u| *u != &state.username)
                                .cloned()
                                .collect();
                            match key.code {
                                KeyCode::Up => {
                                    if state.selected_index > 0 {
                                        state.selected_index -= 1;
                                    }
                                }
                                KeyCode::Down => {
                                    if !filtered.is_empty()
                                        && state.selected_index < filtered.len() - 1
                                    {
                                        state.selected_index += 1;
                                    }
                                }
                                KeyCode::Enter => {
                                    if !filtered.is_empty() {
                                        let target = filtered[state.selected_index].clone();
                                        state.screen = AppScreen::Calling { target: target.clone() };
                                        if let Some(lobby) = &state.livekit_lobby {
                                            let req = SignalingMessage::CallRequest {
                                                from: state.username.clone(),
                                                to: target,
                                            };
                                            let _ = send_signaling(lobby, &req).await;
                                        }
                                    }
                                }
                                KeyCode::Char('j') => {
                                    state.input_buffer.clear();
                                    state.screen = AppScreen::JoinRoom;
                                }
                                KeyCode::Char('s') => {
                                    state.active_input_index = 0;
                                    state.screen = AppScreen::Settings;
                                }
                                KeyCode::Char('q') | KeyCode::Esc => break,
                                _ => {}
                            }
                        }
                        AppScreen::Ringing { caller } => {
                            let caller = caller.clone();
                            match key.code {
                                KeyCode::Char('y') => {
                                    // Accept: create call room, send CallAccepted, join room
                                    let room_name = format!("call_{}_{}", caller, state.username);
                                    let my_name = state.username.clone();
                                    match create_token(&state.api_key, &state.api_secret, &my_name, &room_name) {
                                        Ok(token) => {
                                            match Room::connect(&state.livekit_url, &token, RoomOptions::default()).await {
                                                Ok((call_room, rx_call)) => {
                                                    // Notify caller
                                                    if let Some(lobby) = &state.livekit_lobby {
                                                        let accepted = SignalingMessage::CallAccepted {
                                                            from: my_name.clone(),
                                                            to: caller.clone(),
                                                            room: room_name.clone(),
                                                        };
                                                        let _ = send_signaling(lobby, &accepted).await;
                                                    }
                                                    state.screen = AppScreen::InCall;
                                                    audio_pub = audio::setup_microphone(&call_room, state.audio_input_level.clone()).await;
                                                    video::setup_camera(&call_room, state.local_video_frame.clone()).await;
                                                    events::handle_room_events(rx_call, state.remote_video_frames.clone(), state.audio_output_level.clone());
                                                    state.livekit_room = Some(call_room);
                                                }
                                                Err(e) => {
                                                    state.screen = AppScreen::Error(format!(
                                                        "Failed to join call room: {}", e
                                                    ));
                                                }
                                            }
                                        }
                                        Err(e) => {
                                            state.screen = AppScreen::Error(format!("Token error: {}", e));
                                        }
                                    }
                                }
                                KeyCode::Char('n') => {
                                    if let Some(lobby) = &state.livekit_lobby {
                                        let reject = SignalingMessage::CallRejected {
                                            from: state.username.clone(),
                                            to: caller.clone(),
                                        };
                                        let _ = send_signaling(lobby, &reject).await;
                                    }
                                    state.screen = AppScreen::ContactList;
                                }
                                KeyCode::Char('q') | KeyCode::Esc => break,
                                _ => {}
                            }
                        }
                        AppScreen::Calling { .. } => {
                            if key.code == KeyCode::Char('q') || key.code == KeyCode::Esc {
                                state.screen = AppScreen::ContactList;
                            }
                        }
                        AppScreen::InCall => match key.code {
                            KeyCode::Char('q') | KeyCode::Esc => {
                                if let Some(r) = state.livekit_room.take() {
                                    let _ = r.close().await;
                                }
                                state.screen = AppScreen::ContactList;
                            }
                            KeyCode::Char('m') => {
                                state.is_muted = !state.is_muted;
                                if let Some(ref pub_track) = audio_pub {
                                    if state.is_muted {
                                        pub_track.mute();
                                    } else {
                                        pub_track.unmute();
                                    }
                                }
                            }
                            KeyCode::Char('r') => {
                                state.render_mode = match state.render_mode {
                                    livekit_tui_client::app_state::RenderMode::Braille => livekit_tui_client::app_state::RenderMode::HalfBlock,
                                    livekit_tui_client::app_state::RenderMode::HalfBlock => livekit_tui_client::app_state::RenderMode::Braille,
                                };
                            }
                            _ => {}
                        },
                        AppScreen::JoinRoom => {
                            match key.code {
                                KeyCode::Enter => {
                                    let room_name = state.input_buffer.trim().to_string();
                                    if !room_name.is_empty() {
                                        match create_token(&state.api_key, &state.api_secret, &state.username, &room_name) {
                                            Ok(token) => {
                                                match Room::connect(&state.livekit_url, &token, RoomOptions::default()).await {
                                                    Ok((call_room, rx_call)) => {
                                                        state.screen = AppScreen::InCall;
                                                        audio_pub = audio::setup_microphone(&call_room, state.audio_input_level.clone()).await;
                                                        video::setup_camera(&call_room, state.local_video_frame.clone()).await;
                                                        events::handle_room_events(rx_call, state.remote_video_frames.clone(), state.audio_output_level.clone());
                                                        state.livekit_room = Some(call_room);
                                                    }
                                                    Err(e) => {
                                                        state.screen = AppScreen::Error(format!("Failed to join room: {}", e));
                                                    }
                                                }
                                            }
                                            Err(e) => {
                                                state.screen = AppScreen::Error(format!("Token error: {}", e));
                                            }
                                        }
                                    }
                                }
                                KeyCode::Esc => {
                                    state.screen = AppScreen::ContactList;
                                }
                                KeyCode::Char(c) => {
                                    state.input_buffer.push(c);
                                }
                                KeyCode::Backspace => {
                                    state.input_buffer.pop();
                                }
                                _ => {}
                            }
                        }
                        AppScreen::Settings => {
                            match key.code {
                                KeyCode::Tab | KeyCode::Down => {
                                    state.active_input_index = (state.active_input_index + 1) % 4;
                                }
                                KeyCode::BackTab | KeyCode::Up => {
                                    state.active_input_index = (state.active_input_index + 3) % 4;
                                }
                                KeyCode::Left | KeyCode::Right => {
                                    if state.active_input_index == 3 {
                                        state.render_mode = match state.render_mode {
                                            livekit_tui_client::app_state::RenderMode::Braille => livekit_tui_client::app_state::RenderMode::HalfBlock,
                                            livekit_tui_client::app_state::RenderMode::HalfBlock => livekit_tui_client::app_state::RenderMode::Braille,
                                        };
                                    }
                                }
                                KeyCode::Enter => {
                                    let mode_str = match state.render_mode {
                                        livekit_tui_client::app_state::RenderMode::Braille => "braille".to_string(),
                                        livekit_tui_client::app_state::RenderMode::HalfBlock => "halfblock".to_string(),
                                    };
                                    // Save and go back
                                    let _ = config::save(&livekit_tui_client::config::Config {
                                        livekit_url: state.livekit_url.clone(),
                                        api_key: state.api_key.clone(),
                                        api_secret: state.api_secret.clone(),
                                        last_username: state.username.clone(),
                                        render_mode: Some(mode_str),
                                    });
                                    state.active_input_index = 0;
                                    state.screen = AppScreen::ContactList;
                                }
                                KeyCode::Esc => {
                                    state.active_input_index = 0;
                                    state.screen = AppScreen::ContactList;
                                }
                                KeyCode::Char(c) => {
                                    match state.active_input_index {
                                        0 => state.livekit_url.push(c),
                                        1 => state.api_key.push(c),
                                        2 => state.api_secret.push(c),
                                        _ => {}
                                    }
                                }
                                KeyCode::Backspace => {
                                    match state.active_input_index {
                                        0 => { state.livekit_url.pop(); }
                                        1 => { state.api_key.pop(); }
                                        2 => { state.api_secret.pop(); }
                                        _ => {}
                                    }
                                }
                                _ => {}
                            }
                        }
                        AppScreen::Error(_) => {
                            state.screen = AppScreen::ContactList;
                        }
                        AppScreen::Login => unreachable!(),
                    }
                }
                Event::Paste(text) => {
                    if let AppScreen::Settings = state.screen {
                        let safe_text = text.replace('\n', "").replace('\r', "");
                        match state.active_input_index {
                            0 => state.livekit_url.push_str(&safe_text),
                            1 => state.api_key.push_str(&safe_text),
                            2 => state.api_secret.push_str(&safe_text),
                            _ => {}
                        }
                    }
                }
                _ => {}
            }
        }
        tokio::time::sleep(Duration::from_millis(16)).await;
    }

    // Cleanup
    if let Some(room) = state.livekit_room {
        let _ = room.close().await;
    }
    if let Some(lobby) = state.livekit_lobby {
        let _ = lobby.close().await;
    }
    stdout().execute(crossterm::event::DisableBracketedPaste)?;
    disable_raw_mode()?;
    stdout().execute(LeaveAlternateScreen)?;
    Ok(())
}

// ── Lobby event handler ───────────────────────────────────────────────────────

async fn handle_lobby_events(
    mut rx: tokio::sync::mpsc::UnboundedReceiver<RoomEvent>,
    tx_sig: tokio::sync::mpsc::UnboundedSender<SignalingMessage>,
    participant_list: Arc<Mutex<Vec<String>>>,
    _my_name: String,
) {
    while let Some(event) = rx.recv().await {
        match event {
            RoomEvent::ParticipantConnected(participant) => {
                let id = participant.identity().as_str().to_string();
                let mut list = participant_list.lock().unwrap();
                if !list.contains(&id) {
                    list.push(id);
                }
            }
            RoomEvent::ParticipantDisconnected(participant) => {
                let id = participant.identity().as_str().to_string();
                let mut list = participant_list.lock().unwrap();
                list.retain(|u| u != &id);
            }
            RoomEvent::DataReceived { payload, .. } => {
                if let Ok(text) = std::str::from_utf8(&payload) {
                    if let Ok(msg) = serde_json::from_str::<SignalingMessage>(text) {
                        let _ = tx_sig.send(msg);
                    }
                }
            }
            _ => {}
        }
    }
}

