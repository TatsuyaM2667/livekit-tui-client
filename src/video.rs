use crate::utils::{i420_to_rgb, rgba_to_i420};
use futures_util::StreamExt;
use livekit::options::TrackPublishOptions;
use livekit::prelude::*;
use livekit::webrtc::prelude::*;
use livekit::webrtc::video_source::native::NativeVideoSource;
use livekit::webrtc::video_stream::native::NativeVideoStream;
use nokhwa::pixel_format::RgbAFormat;
use nokhwa::utils::{CameraIndex, RequestedFormat, RequestedFormatType};
use nokhwa::Camera;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

pub async fn setup_camera(
    room: &Room,
    local_frame: Arc<Mutex<Option<(Vec<u8>, u32, u32)>>>,
) {
    let video_source = NativeVideoSource::new(
        VideoResolution {
            width: 320,
            height: 240,
        },
        false,
    );

    let video_track = LocalVideoTrack::create_video_track(
        "camera",
        RtcVideoSource::Native(video_source.clone()),
    );

    let _ = room
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

                        // Store local preview for self-view
                        {
                            let rgb: Vec<u8> = rgba_raw.chunks(4).flat_map(|c| c[..3].iter().copied()).collect();
                            let mut lf = local_frame.lock().unwrap();
                            *lf = Some((rgb, w, h));
                        }

                        let i420 = rgba_to_i420(&rgba_raw, w, h);

                        let video_frame = VideoFrame {
                            rotation: VideoRotation::VideoRotation0,
                            timestamp_us: 0,
                            frame_metadata: None,
                            buffer: i420,
                        };
                        video_source.capture_frame(&video_frame);
                    }
                    std::thread::sleep(Duration::from_millis(100));
                }
            }
        }
    });
}

pub fn spawn_video_listener_task(
    video_track: RemoteVideoTrack,
    participant_identity: String,
    video_frames: Arc<Mutex<HashMap<String, (Vec<u8>, u32, u32)>>>,
) {
    tokio::spawn(async move {
        let mut native_stream = NativeVideoStream::new(video_track.rtc_track());
        while let Some(frame) = native_stream.next().await {
            let buffer = frame.buffer;
            let i420 = buffer.to_i420();
            let width = i420.width();
            let height = i420.height();
            let rgb = i420_to_rgb(&i420, width, height);

            let mut lock = video_frames.lock().unwrap();
            lock.insert(participant_identity.clone(), (rgb, width, height));
        }
    });
}
