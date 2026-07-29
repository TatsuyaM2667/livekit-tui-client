use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use futures_util::StreamExt;
use livekit::options::TrackPublishOptions;
use livekit::prelude::*;
use livekit::webrtc::audio_source::native::NativeAudioSource;
use livekit::webrtc::audio_stream::native::NativeAudioStream;
use livekit::webrtc::prelude::*;
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

pub async fn setup_microphone(room: &Room) -> Option<LocalTrackPublication> {
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

    let pub_res = room
        .local_participant()
        .publish_track(
            LocalTrack::Audio(audio_track),
            TrackPublishOptions::default(),
        )
        .await;

    let publication = pub_res.ok()?;
    publication.unmute();

    let host = cpal::default_host();
    if let Some(device) = host.default_input_device() {
        if let Ok(config) = device.default_input_config() {
            let sample_rate = config.sample_rate().0;
            let channels = config.channels() as u32;
            let audio_src = audio_source.clone();

            let err_fn = move |err| eprintln!("Audio capture error: {}", err);

            let input_stream = match config.sample_format() {
                cpal::SampleFormat::F32 => device.build_input_stream(
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
                    err_fn,
                    None,
                ),
                cpal::SampleFormat::I16 => device.build_input_stream(
                    &config.into(),
                    move |data: &[i16], _: &_| {
                        let samples_per_channel = (data.len() as u32) / channels;
                        let _ = audio_src.capture_frame(&AudioFrame {
                            data: data.to_vec().into(),
                            sample_rate,
                            num_channels: channels,
                            samples_per_channel,
                        });
                    },
                    err_fn,
                    None,
                ),
                _ => return Some(publication),
            };

            if let Ok(stream) = input_stream {
                let _ = stream.play();
                std::mem::forget(stream);
            }
        }
    }

    Some(publication)
}

pub fn spawn_speaker_task(audio_track: RemoteAudioTrack) {
    let handle = tokio::runtime::Handle::current();
    std::thread::spawn(move || {
        handle.block_on(async move {
            let host = cpal::default_host();

            if let Some(device) = host.default_output_device() {
                if let Ok(config) = device.default_output_config() {
                    let sample_rate = config.sample_rate().0;
                    let channels = config.channels();

                    // Ask LiveKit to resample to the device's exact rate and channels
                    let mut native_stream = NativeAudioStream::new(
                        audio_track.rtc_track(),
                        sample_rate as i32,
                        channels as i32,
                    );

                    let sample_buffer = Arc::new(Mutex::new(VecDeque::<f32>::new()));
                    
                    let err_fn = move |err| eprintln!("Audio output error: {}", err);
                    
                    let stream_res = match config.sample_format() {
                        cpal::SampleFormat::F32 => {
                            let buf_clone = sample_buffer.clone();
                            device.build_output_stream(
                                &config.into(),
                                move |data: &mut [f32], _: &_| {
                                    let mut buf = buf_clone.lock().unwrap();
                                    for sample in data.iter_mut() {
                                        *sample = buf.pop_front().unwrap_or(0.0);
                                    }
                                },
                                err_fn,
                                None,
                            )
                        }
                        cpal::SampleFormat::I16 => {
                            let buf_clone = sample_buffer.clone();
                            device.build_output_stream(
                                &config.into(),
                                move |data: &mut [i16], _: &_| {
                                    let mut buf = buf_clone.lock().unwrap();
                                    for sample in data.iter_mut() {
                                        let float_sample = buf.pop_front().unwrap_or(0.0);
                                        *sample = (float_sample.clamp(-1.0, 1.0) * 32767.0) as i16;
                                    }
                                },
                                err_fn,
                                None,
                            )
                        }
                        _ => return,
                    };

                    if let Ok(stream) = stream_res {
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
