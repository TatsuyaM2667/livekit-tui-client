use crate::audio::spawn_speaker_task;
use crate::video::spawn_video_listener_task;
use livekit::prelude::*;
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc::UnboundedReceiver;

pub fn handle_room_events(
    mut rx: UnboundedReceiver<RoomEvent>,
    video_frame_clone: Arc<Mutex<Option<(Vec<u8>, u32, u32)>>>,
    output_level: Arc<Mutex<f32>>,
) {
    tokio::spawn(async move {
        while let Some(event) = rx.recv().await {
            if let RoomEvent::TrackSubscribed { track, .. } = event {
                if let RemoteTrack::Audio(audio_track) = track {
                    spawn_speaker_task(audio_track, output_level.clone());
                } else if let RemoteTrack::Video(video_track) = track {
                    spawn_video_listener_task(video_track, video_frame_clone.clone());
                }
            }
        }
    });
}
