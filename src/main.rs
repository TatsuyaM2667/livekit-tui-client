use anyhow::Result;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use crossterm::{
    event::{self, Event, KeyCode},
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    ExecutableCommand,
};
use futures_util::StreamExt;
use livekit::options::TrackPublishOptions;
use livekit::prelude::*;
use livekit::webrtc::audio_source::native::NativeAudioSource;
use livekit::webrtc::audio_stream::native::NativeAudioStream;
use livekit::webrtc::prelude::*;
use livekit::webrtc::video_source::native::NativeVideoSource;
use nokhwa::pixel_format::RgbAFormat;
use nokhwa::utils::{CameraIndex, RequestedFormat, RequestedFormatType};
use nokhwa::Camera;
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, List, ListItem, Paragraph},
    Terminal,
};
use std::collections::VecDeque;
use std::io::stdout;
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// Helper: Converts RGBA image bytes to YUV420 (I420) for WebRTC video frames
fn rgba_to_i420(rgba: &[u8], width: u32, height: u32) -> I420Buffer {
    let mut i420 = I420Buffer::new(width, height);
    let (stride_y, stride_u, stride_v) = i420.strides();
    let (data_y, data_u, data_v) = i420.data_mut();

    let w = width as usize;
    let h = height as usize;

    for y in 0..h {
        for x in 0..w {
            let i = (y * w + x) * 4;
            let r = rgba[i] as f32;
            let g = rgba[i + 1] as f32;
            let b = rgba[i + 2] as f32;

            let y_val = (0.257 * r + 0.504 * g + 0.098 * b + 16.0) as u8;
            data_y[y * (stride_y as usize) + x] = y_val;

            if y % 2 == 0 && x % 2 == 0 {
                let u_val = (-0.148 * r - 0.291 * g + 0.439 * b + 128.0) as u8;
                let v_val = (0.439 * r - 0.368 * g + 0.071 * b + 128.0) as u8;
                let uv_x = x / 2;
                let uv_y = y / 2;
                data_u[uv_y * (stride_u as usize) + uv_x] = u_val;
                data_v[uv_y * (stride_v as usize) + uv_x] = v_val;
            }
        }
    }
    i420
}

#[tokio::main]
async fn main() -> Result<()> {
    let server_url = "wss://tatsuya-almaserver.tailc98924.ts.net";
    let token = "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJpc3MiOiJkZXZrZXkiLCJzdWIiOiJhcmNoLXVzZXIiLCJleHAiOjE3ODUxMjg5MDksIm5iZiI6MTc4NTA0MjUwOSwiaWF0IjoxNzg1MDQyNTA5LCJpZGVudGl0eSI6ImFyY2gtdXNlciIsIm5hbWUiOiJhcmNoLXVzZXIiLCJ2aWRlbyI6eyJyb29tSm9pbiI6dHJ1ZSwicm9vbSI6InRlc3Qtcm9vbSJ9fQ.6PBMFKDUSiRqiiYjEXdR4PRUUH55uSdMqBLNkkEe6WE";

    enable_raw_mode()?;
    stdout().execute(EnterAlternateScreen)?;
    let mut terminal = Terminal::new(CrosstermBackend::new(stdout()))?;

    // 1. LiveKit Room Connection
    let room_result = Room::connect(server_url, token, RoomOptions::default()).await;

    let mut error_msg = String::new();
    let (room, mut rx) = match room_result {
        Ok((room, rx)) => (Some(room), rx),
        Err(e) => {
            error_msg = e.to_string();
            (None, tokio::sync::mpsc::unbounded_channel().1)
        }
    };

    let mut audio_pub = None;
    let mut is_muted = false;

    if let Some(ref r) = room {
        // --- 2. Setup Microphone Capture (CPAL -> NativeAudioSource -> LiveKit) ---
        let audio_source = NativeAudioSource::new(
            AudioSourceOptions {
                echo_cancellation: true,
                noise_suppression: true,
                auto_gain_control: true,
            },
            48000, // sample_rate
            1,     // num_channels
            1000,  // queue_size_ms
        );

        let audio_track = LocalAudioTrack::create_audio_track(
            "microphone",
            RtcAudioSource::Native(audio_source.clone()),
        );

        let pub_res = r
            .local_participant()
            .publish_track(
                LocalTrack::Audio(audio_track),
                TrackPublishOptions::default(),
            )
            .await;

        if let Ok(p) = pub_res {
            p.unmute(); // 同期メソッド呼び出し
            audio_pub = Some(p);
        }

        // Spawn Microphone Capture Loop
        let host = cpal::default_host();
        if let Some(device) = host.default_input_device() {
            if let Ok(config) = device.default_input_config() {
                let sample_rate = config.sample_rate().0;
                let channels = config.channels() as u32;
                let audio_src = audio_source.clone();

                let input_stream = device.build_input_stream(
                    &config.into(),
                    move |data: &[f32], _: &_| {
                        let pcm16: Vec<i16> = data
                            .iter()
                            .map(|&s| (s.clamp(-1.0, 1.0) * 32767.0) as i16)
                            .collect();
                        let samples_per_channel = (pcm16.len() as u32) / channels;
                        let _ = audio_src.capture_frame(&AudioFrame {
                            data: pcm16.into(),
                            sample_rate,
                            num_channels: channels,
                            samples_per_channel,
                        });
                    },
                    move |err| eprintln!("Audio capture error: {}", err),
                    None,
                );

                if let Ok(stream) = input_stream {
                    let _ = stream.play();
                    std::mem::forget(stream); // Keep audio stream active
                }
            }
        }

        // --- 3. Setup Webcam Capture (Nokhwa -> NativeVideoSource -> LiveKit) ---
        let video_source = NativeVideoSource::new(
            VideoResolution {
                width: 640,
                height: 480,
            },
            false,
        );

        let video_track = LocalVideoTrack::create_video_track(
            "camera",
            RtcVideoSource::Native(video_source.clone()),
        );

        let _ = r
            .local_participant()
            .publish_track(
                LocalTrack::Video(video_track),
                TrackPublishOptions::default(),
            )
            .await;

        // Spawn Camera Capture Thread
        tokio::task::spawn_blocking(move || {
            let index = CameraIndex::Index(0);
            let requested =
                RequestedFormat::new::<RgbAFormat>(RequestedFormatType::AbsoluteHighestFrameRate);
            if let Ok(mut camera) = Camera::new(index, requested) {
                if camera.open_stream().is_ok() {
                    while let Ok(frame) = camera.frame() {
                        if let Ok(buffer) = frame.decode_image::<RgbAFormat>() {
                            let (w, h) = (buffer.width(), buffer.height());
                            let rgba_raw = buffer.into_raw();
                            let i420 = rgba_to_i420(&rgba_raw, w, h);

                            let video_frame = VideoFrame {
                                rotation: VideoRotation::VideoRotation0,
                                timestamp_us: 0,
                                frame_metadata: None,
                                buffer: i420,
                            };
                            video_source.capture_frame(&video_frame);
                        }
                        std::thread::sleep(Duration::from_millis(33)); // ~30 FPS
                    }
                }
            }
        });
    }

    // --- 4. Incoming Audio Subscriptions (Speaker Output) ---
    tokio::spawn(async move {
        while let Some(event) = rx.recv().await {
            if let RoomEvent::TrackSubscribed { track, .. } = event {
                if let RemoteTrack::Audio(audio_track) = track {
                    let handle = tokio::runtime::Handle::current();
                    std::thread::spawn(move || {
                        handle.block_on(async move {
                            let mut native_stream =
                                NativeAudioStream::new(audio_track.rtc_track(), 48000, 1);
                            let host = cpal::default_host();

                            if let Some(device) = host.default_output_device() {
                                if let Ok(config) = device.default_output_config() {
                                    let sample_buffer =
                                        Arc::new(Mutex::new(VecDeque::<f32>::new()));
                                    let buf_clone = sample_buffer.clone();

                                    let stream = device.build_output_stream(
                                        &config.into(),
                                        move |data: &mut [f32], _: &_| {
                                            let mut buf = buf_clone.lock().unwrap();
                                            for sample in data.iter_mut() {
                                                *sample = buf.pop_front().unwrap_or(0.0);
                                            }
                                        },
                                        move |err| eprintln!("Audio output error: {}", err),
                                        None,
                                    );

                                    if let Ok(stream) = stream {
                                        let _ = stream.play();
                                        while let Some(frame) = native_stream.next().await {
                                            let mut buf = sample_buffer.lock().unwrap();
                                            for &s in frame.data.iter() {
                                                buf.push_back((s as f32) / 32768.0);
                                            }
                                        }
                                    }
                                }
                            }
                        });
                    });
                }
            }
        }
    });

    // --- 5. TUI Rendering & Event Loop ---
    loop {
        terminal.draw(|frame| {
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
                let mic_status = if is_muted {
                    "OFF (Muted)"
                } else {
                    "ON (Active)"
                };
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
                        .fg(if is_muted {
                            Color::Yellow
                        } else {
                            Color::Green
                        })
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
            if let Some(ref r) = room {
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

            let info_text = if room.is_some() {
                format!(
                    "LiveKit Voice & Camera Streaming Active!\n\n\
                     - [m] : Toggle Mute\n\
                     - [q] : Quit\n\n\
                     Users in Room: {}",
                    room.as_ref()
                        .map(|r| r.remote_participants().len() + 1)
                        .unwrap_or(0)
                )
            } else {
                "Disconnected".to_string()
            };

            let info_widget = Paragraph::new(info_text)
                .block(Block::default().title(" Status ").borders(Borders::ALL));
            frame.render_widget(info_widget, body_chunks[1]);

            let footer = Paragraph::new(" [m] Toggle Mute  |  [q] Quit App")
                .style(Style::default().fg(Color::DarkGray))
                .block(Block::default().borders(Borders::ALL));
            frame.render_widget(footer, main_chunks[2]);
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
