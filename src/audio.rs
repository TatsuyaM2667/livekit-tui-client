use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use futures_util::StreamExt;
use livekit::options::TrackPublishOptions;
use livekit::prelude::*;
use livekit::webrtc::audio_source::native::NativeAudioSource;
use livekit::webrtc::audio_stream::native::NativeAudioStream;
use livekit::webrtc::prelude::*;
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

fn compute_rms_level_i16(data: &[i16]) -> f32 {
    if data.is_empty() {
        return 0.0;
    }
    let sum_sq: f32 = data.iter().map(|&s| {
        let f = s as f32 / 32768.0;
        f * f
    }).sum();
    (sum_sq / data.len() as f32).sqrt()
}

fn compute_rms_level_f32(data: &[f32]) -> f32 {
    if data.is_empty() {
        return 0.0;
    }
    let sum_sq: f32 = data.iter().map(|&s| s * s).sum();
    (sum_sq / data.len() as f32).sqrt()
}

pub async fn setup_microphone(
    room: &Room,
    input_level: Arc<Mutex<f32>>,
) -> Option<LocalTrackPublication> {
    let audio_source = NativeAudioSource::new(
        AudioSourceOptions {
            echo_cancellation: true,
            noise_suppression: true,
            auto_gain_control: true,
        },
        48000,
        1,
        1000,
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

    let publication = match pub_res {
        Ok(p) => {
            p.unmute();
            p
        }
        Err(e) => {
            eprintln!("[audio] Failed to publish microphone track: {}", e);
            return None;
        }
    };

    let host = cpal::default_host();
    let device = match host.default_input_device() {
        Some(d) => d,
        None => {
            eprintln!("[audio] No default input device found");
            return Some(publication);
        }
    };

    let config = match device.default_input_config() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("[audio] Failed to get default input config: {}", e);
            return Some(publication);
        }
    };

    let sample_rate = config.sample_rate().0;
    let channels = config.channels() as u32;
    let audio_src = audio_source.clone();
    let err_fn = move |err| eprintln!("[audio] Capture stream error: {}", err);

    let input_stream = match config.sample_format() {
        cpal::SampleFormat::F32 => {
            let level = input_level.clone();
            device.build_input_stream(
                &config.into(),
                move |data: &[f32], _: &_| {
                    let rms = compute_rms_level_f32(data);
                    {
                        let mut l = level.lock().unwrap();
                        *l = rms;
                    }
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
            )
        }
        cpal::SampleFormat::I16 => {
            let level = input_level.clone();
            device.build_input_stream(
                &config.into(),
                move |data: &[i16], _: &_| {
                    let rms = compute_rms_level_i16(data);
                    {
                        let mut l = level.lock().unwrap();
                        *l = rms;
                    }
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
            )
        }
        cpal::SampleFormat::U16 => {
            let level = input_level.clone();
            device.build_input_stream(
                &config.into(),
                move |data: &[u16], _: &_| {
                    let pcm16: Vec<i16> = data
                        .iter()
                        .map(|&s| (s as i32 - 32768).clamp(-32768, 32767) as i16)
                        .collect();
                    let rms = compute_rms_level_i16(&pcm16);
                    {
                        let mut l = level.lock().unwrap();
                        *l = rms;
                    }
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
            )
        }
        other => {
            eprintln!("[audio] Unsupported input sample format: {:?}", other);
            return Some(publication);
        }
    };

    match input_stream {
        Ok(stream) => {
            if let Err(e) = stream.play() {
                eprintln!("[audio] Failed to play input stream: {}", e);
            } else {
                std::mem::forget(stream);
            }
        }
        Err(e) => {
            eprintln!("[audio] Failed to build input stream: {}", e);
        }
    }

    Some(publication)
}

pub fn spawn_speaker_task(
    audio_track: RemoteAudioTrack,
    output_level: Arc<Mutex<f32>>,
) {
    let handle = tokio::runtime::Handle::current();
    std::thread::spawn(move || {
        handle.block_on(async move {
            let host = cpal::default_host();

            let device = match host.default_output_device() {
                Some(d) => d,
                None => {
                    eprintln!("[audio] No default output device found");
                    return;
                }
            };

            let config = match device.default_output_config() {
                Ok(c) => c,
                Err(e) => {
                    eprintln!("[audio] Failed to get default output config: {}", e);
                    return;
                }
            };

            let sample_rate = config.sample_rate().0;
            let channels = config.channels();

            let mut native_stream = NativeAudioStream::new(
                audio_track.rtc_track(),
                sample_rate as i32,
                channels as i32,
            );

            let sample_buffer = Arc::new(Mutex::new(VecDeque::<f32>::new()));
            let err_fn = move |err| eprintln!("[audio] Output stream error: {}", err);

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
                cpal::SampleFormat::U16 => {
                    let buf_clone = sample_buffer.clone();
                    device.build_output_stream(
                        &config.into(),
                        move |data: &mut [u16], _: &_| {
                            let mut buf = buf_clone.lock().unwrap();
                            for sample in data.iter_mut() {
                                let float_sample = buf.pop_front().unwrap_or(0.0);
                                *sample = ((float_sample.clamp(-1.0, 1.0) + 1.0) * 32767.5) as u16;
                            }
                        },
                        err_fn,
                        None,
                    )
                }
                other => {
                    eprintln!("[audio] Unsupported output sample format: {:?}", other);
                    return;
                }
            };

            let level_ref = output_level.clone();
            match stream_res {
                Ok(stream) => {
                    if let Err(e) = stream.play() {
                        eprintln!("[audio] Failed to play output stream: {}", e);
                        return;
                    }
                    while let Some(frame) = native_stream.next().await {
                        let rms = compute_rms_level_i16(&frame.data);
                        {
                            let mut l = level_ref.lock().unwrap();
                            *l = rms;
                        }
                        let mut buf = sample_buffer.lock().unwrap();
                        for &s in frame.data.iter() {
                            buf.push_back((s as f32) / 32768.0);
                        }
                    }
                }
                Err(e) => {
                    eprintln!("[audio] Failed to build output stream: {}", e);
                }
            }
        });
    });
}
