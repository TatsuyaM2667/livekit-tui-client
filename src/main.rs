mod audio;
mod events;
mod tui;
mod utils;
mod video;

use anyhow::Result;
use crossterm::{
    event::{self, Event, KeyCode},
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    ExecutableCommand,
};
use livekit::prelude::*;
use ratatui::{backend::CrosstermBackend, Terminal};
use std::env;
use std::io::stdout;
use std::sync::{Arc, Mutex};
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<()> {
    // .env ファイルの読み込み
    dotenvy::dotenv().ok();

    let server_url =
        env::var("LIVEKIT_URL").expect("LIVEKIT_URL is not set in .env or environment variables");
    let token = env::var("LIVEKIT_TOKEN")
        .expect("LIVEKIT_TOKEN is not set in .env or environment variables");

    enable_raw_mode()?;
    stdout().execute(EnterAlternateScreen)?;
    let mut terminal = Terminal::new(CrosstermBackend::new(stdout()))?;

    // 1. LiveKit Room Connection
    let room_result = Room::connect(&server_url, &token, RoomOptions::default()).await;

    let mut error_msg = String::new();
    let (room, rx) = match room_result {
        Ok((room, rx)) => (Some(room), rx),
        Err(e) => {
            error_msg = e.to_string();
            (None, tokio::sync::mpsc::unbounded_channel().1)
        }
    };

    let mut audio_pub = None;
    let mut is_muted = false;

    // Shared video frame buffer: Option<(RGB_Data, Width, Height)>
    let remote_video_frame: Arc<Mutex<Option<(Vec<u8>, u32, u32)>>> = Arc::new(Mutex::new(None));

    if let Some(ref r) = room {
        // Setup Microphone Capture
        audio_pub = audio::setup_microphone(r).await;

        // Setup Webcam Capture
        video::setup_camera(r).await;

        // Handle Incoming Events (Audio/Video Subscriptions)
        events::handle_room_events(rx, remote_video_frame.clone());
    }

    // TUI Rendering & Event Loop
    loop {
        terminal.draw(|frame| {
            tui::render_ui(
                frame,
                room.as_ref(),
                is_muted,
                &error_msg,
                &remote_video_frame,
            );
        })?;

        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                match key.code {
                    KeyCode::Char('q') => break,
                    KeyCode::Char('m') => {
                        is_muted = !is_muted;
                        if let Some(ref pub_track) = audio_pub {
                            if is_muted {
                                pub_track.mute();
                            } else {
                                pub_track.unmute();
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    if let Some(room) = room {
        room.close().await?;
    }
    disable_raw_mode()?;
    stdout().execute(LeaveAlternateScreen)?;
    Ok(())
}
