use crate::audio::spawn_speaker_task;
use crate::video::spawn_video_listener_task;
use livekit::prelude::*;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc::UnboundedReceiver;

pub fn handle_room_events(
    mut rx: UnboundedReceiver<RoomEvent>,
    video_frames: Arc<Mutex<HashMap<String, (Vec<u8>, u32, u32)>>>,
    output_level: Arc<Mutex<f32>>,
) {
    tokio::spawn(async move {
        while let Some(event) = rx.recv().await {
            match event {
                RoomEvent::TrackSubscribed { track, participant, .. } => {
                    let identity = participant.identity().as_str().to_string();
                    if let RemoteTrack::Audio(audio_track) = track {
                        spawn_speaker_task(audio_track, output_level.clone());
                    } else if let RemoteTrack::Video(video_track) = track {
                        spawn_video_listener_task(video_track, identity, video_frames.clone());
                    }
                }
                RoomEvent::TrackUnsubscribed { track, participant, .. } => {
                    if let RemoteTrack::Video(_) = track {
                        let identity = participant.identity().as_str().to_string();
                        let mut frames = video_frames.lock().unwrap();
                        frames.remove(&identity);
                    }
                }
                _ => {}
            }
        }
    });
}
