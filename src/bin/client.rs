use anyhow::Result;
use crossterm::{
    event::{self, Event, KeyCode},
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    ExecutableCommand,
};
use futures_util::{SinkExt, StreamExt};
use livekit::prelude::*;
use livekit_tui_client::{
    app_state::{AppScreen, AppState},
    audio, events,
    shared::{ClientMessage, ServerMessage},
    tui, video,
};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::env;
use std::io::stdout;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();

    enable_raw_mode()?;
    stdout().execute(EnterAlternateScreen)?;
    let mut terminal = Terminal::new(CrosstermBackend::new(stdout()))?;

    let mut state = AppState::new();
    let mut audio_pub = None;

    let ws_url = env::var("SIGNALING_URL").unwrap_or_else(|_| "ws://127.0.0.1:3000/ws".to_string());
    
    // Connect to signaling server
    let (ws_stream, _) = connect_async(&ws_url).await?;
    let (mut ws_sender, mut ws_receiver) = ws_stream.split();

    let (tx_ws, mut rx_ws) = mpsc::unbounded_channel::<ClientMessage>();

    // Send loop
    tokio::spawn(async move {
        while let Some(msg) = rx_ws.recv().await {
            if let Ok(text) = serde_json::to_string(&msg) {
                let _ = ws_sender.send(Message::Text(text)).await;
            }
        }
    });

    // Receive loop
    let (tx_in, mut rx_in) = mpsc::unbounded_channel::<ServerMessage>();
    tokio::spawn(async move {
        while let Some(Ok(msg)) = ws_receiver.next().await {
            if let Message::Text(text) = msg {
                if let Ok(server_msg) = serde_json::from_str::<ServerMessage>(&text) {
                    let _ = tx_in.send(server_msg);
                }
            }
        }
    });

    loop {
        // Handle incoming Server messages
        while let Ok(msg) = rx_in.try_recv() {
            match msg {
                ServerMessage::LoginSuccess { username } => {
                    state.username = username;
                    state.screen = AppScreen::ContactList;
                }
                ServerMessage::UserList { users } => {
                    state.users = users;
                    if state.selected_index >= state.users.len() && !state.users.is_empty() {
                        state.selected_index = state.users.len() - 1;
                    }
                }
                ServerMessage::IncomingCall { from_username } => {
                    if state.screen == AppScreen::ContactList {
                        state.screen = AppScreen::Ringing { caller: from_username };
                    } else {
                        // Busy, reject automatically
                        let _ = tx_ws.send(ClientMessage::CallReject { caller_username: from_username });
                    }
                }
                ServerMessage::CallAccepted { room_name: _room_name, token } => {
                    // Connect to LiveKit
                    let lk_url = env::var("LIVEKIT_URL").unwrap_or_else(|_| "wss://your-project.livekit.cloud".to_string());
                    
                    match Room::connect(&lk_url, &token, RoomOptions::default()).await {
                        Ok((room, rx)) => {
                            state.livekit_room = Some(room);
                            state.screen = AppScreen::InCall;

                            // Setup AV
                            if let Some(r) = &state.livekit_room {
                                audio_pub = audio::setup_microphone(r).await;
                                video::setup_camera(r).await;
                                events::handle_room_events(rx, state.remote_video_frame.clone());
                            }
                        }
                        Err(e) => {
                            state.screen = AppScreen::Error(format!("Failed to connect to LiveKit: {}", e));
                        }
                    }
                }
                ServerMessage::CallRejected { target_username } => {
                    if let AppScreen::Calling { target } = &state.screen {
                        if target == &target_username {
                            state.screen = AppScreen::Error(format!("{} rejected the call.", target_username));
                        }
                    }
                }
                ServerMessage::Error { message } => {
                    state.screen = AppScreen::Error(message);
                }
            }
        }

        // Draw UI
        terminal.draw(|frame| {
            tui::render_ui(frame, &state);
        })?;

        // Handle Input
        if event::poll(Duration::from_millis(50))? {
            if let Event::Key(key) = event::read()? {
                match &state.screen {
                    AppScreen::Login => {
                        match key.code {
                            KeyCode::Enter => {
                                if !state.input_buffer.trim().is_empty() {
                                    let _ = tx_ws.send(ClientMessage::Login {
                                        username: state.input_buffer.trim().to_string(),
                                    });
                                }
                            }
                            KeyCode::Char(c) => state.input_buffer.push(c),
                            KeyCode::Backspace => {
                                state.input_buffer.pop();
                            }
                            KeyCode::Esc => break,
                            _ => {}
                        }
                    }
                    AppScreen::ContactList => {
                        let filtered_users: Vec<&String> = state.users.iter().filter(|u| *u != &state.username).collect();
                        match key.code {
                            KeyCode::Up => {
                                if state.selected_index > 0 {
                                    state.selected_index -= 1;
                                }
                            }
                            KeyCode::Down => {
                                if !filtered_users.is_empty() && state.selected_index < filtered_users.len() - 1 {
                                    state.selected_index += 1;
                                }
                            }
                            KeyCode::Enter => {
                                if !filtered_users.is_empty() {
                                    let target = filtered_users[state.selected_index].clone();
                                    state.screen = AppScreen::Calling { target: target.clone() };
                                    let _ = tx_ws.send(ClientMessage::CallRequest { target_username: target });
                                }
                            }
                            KeyCode::Char('q') | KeyCode::Esc => break,
                            _ => {}
                        }
                    }
                    AppScreen::Ringing { caller } => {
                        match key.code {
                            KeyCode::Char('y') => {
                                let _ = tx_ws.send(ClientMessage::CallAccept { caller_username: caller.clone() });
                                // Wait for CallAccepted message from server
                            }
                            KeyCode::Char('n') => {
                                let _ = tx_ws.send(ClientMessage::CallReject { caller_username: caller.clone() });
                                state.screen = AppScreen::ContactList;
                            }
                            KeyCode::Char('q') | KeyCode::Esc => break,
                            _ => {}
                        }
                    }
                    AppScreen::Calling { .. } => {
                        if key.code == KeyCode::Char('q') || key.code == KeyCode::Esc {
                            break;
                        }
                    }
                    AppScreen::InCall => {
                        match key.code {
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
                            _ => {}
                        }
                    }
                    AppScreen::Error(_) => {
                        state.screen = AppScreen::ContactList;
                    }
                }
            }
        }
    }

    if let Some(room) = state.livekit_room {
        let _ = room.close().await;
    }
    disable_raw_mode()?;
    stdout().execute(LeaveAlternateScreen)?;
    Ok(())
}
