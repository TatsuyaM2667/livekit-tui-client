use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use futures_util::StreamExt;
use livekit::options::TrackPublishOptions;
use livekit::prelude::*;
use livekit::webrtc::audio_source::native::NativeAudioSource;
use livekit::webrtc::audio_stream::native::NativeAudioStream;
use livekit::webrtc::prelude::*;
use std::collections::VecDeque;

// ターミナルUI(ratatui)のレイアウト崩れを防ぐため、audio.rs内の eprintln! を無効化するマクロ
macro_rules! eprintln {
    ($($arg:tt)*) => { () };
}
use std::sync::{Arc, Mutex};

pub fn diagnose_audio() {
    eprintln!("[audio] === Audio system diagnosis ===");
    for &host_id in cpal::available_hosts().iter() {
        let host = match cpal::host_from_id(host_id) {
            Ok(h) => h,
            Err(_) => continue,
        };
        eprintln!("[audio] Host: {:?}", host_id);

        if let Some(device) = host.default_input_device() {
            let name = device.name().unwrap_or_default();
            if let Ok(cfg) = device.default_input_config() {
                eprintln!("[audio]   Default INPUT: {} ({:?} {}Hz {}ch)", name, cfg.sample_format(), cfg.sample_rate().0, cfg.channels());
            } else {
                eprintln!("[audio]   Default INPUT: {} (no default config)", name);
            }
        } else {
            eprintln!("[audio]   No default INPUT device");
        }

        if let Some(device) = host.default_output_device() {
            let name = device.name().unwrap_or_default();
            if let Ok(cfg) = device.default_output_config() {
                eprintln!("[audio]   Default OUTPUT: {} ({:?} {}Hz {}ch)", name, cfg.sample_format(), cfg.sample_rate().0, cfg.channels());
            } else {
                eprintln!("[audio]   Default OUTPUT: {} (no default config)", name);
            }
        } else {
            eprintln!("[audio]   No default OUTPUT device");
        }
    }
    eprintln!("[audio] === End diagnosis ===");
}

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
    // Step 1: Find a working capture device and its config
    let (dev_name, device, dev_config) = match find_input_device() {
        Some(d) => d,
        None => {
            eprintln!("[audio] No working input device found");
            // Still publish a silent track so the other side has something
            let source = NativeAudioSource::new(AudioSourceOptions::default(), 48000, 1, 1000);
            let track = LocalAudioTrack::create_audio_track("microphone", RtcAudioSource::Native(source));
            let pub_res = room.local_participant().publish_track(LocalTrack::Audio(track), TrackPublishOptions::default()).await;
            return match pub_res {
                Ok(p) => { p.unmute(); Some(p) }
                Err(e) => { eprintln!("[audio] Failed to publish: {}", e); None }
            };
        }
    };

    let sample_rate = dev_config.sample_rate().0;
    let num_channels = dev_config.channels() as u32;

    eprintln!("[audio] Using input device: {} ({}Hz {}ch {:?})", dev_name, sample_rate, num_channels, dev_config.sample_format());

    // Step 2: Create source with matching params
    let audio_source = NativeAudioSource::new(
        AudioSourceOptions {
            echo_cancellation: true,
            noise_suppression: true,
            auto_gain_control: true,
        },
        sample_rate,
        num_channels,
        1000,
    );

    // Step 3: Publish track
    let audio_track = LocalAudioTrack::create_audio_track(
        "microphone",
        RtcAudioSource::Native(audio_source.clone()),
    );

    let publication = match room.local_participant().publish_track(
        LocalTrack::Audio(audio_track),
        TrackPublishOptions::default(),
    ).await {
        Ok(p) => { p.unmute(); p }
        Err(e) => { eprintln!("[audio] Failed to publish: {}", e); return None; }
    };

    // Step 4: Start capture
    if !start_capture(&device, &dev_config, &audio_source, &input_level, &dev_name) {
        eprintln!("[audio] Failed to start capture on {}", dev_name);
    }

    Some(publication)
}

fn find_input_device() -> Option<(String, cpal::Device, cpal::SupportedStreamConfig)> {
    // Priority: "default" → "pipewire" → any other device
    for host_id in cpal::available_hosts() {
        let host = match cpal::host_from_id(host_id) {
            Ok(h) => h,
            Err(_) => continue,
        };

        // Try named devices first (no full enumeration = no ALSA errors)
        for preferred in &["default", "pipewire", "sysdefault"] {
            if let Ok(devices) = host.input_devices() {
                for d in devices {
                    if let Ok(name) = d.name() {
                        if name == *preferred {
                            if let Ok(config) = d.default_input_config() {
                                return Some((name, d, config));
                            }
                        }
                    }
                }
            }
        }
    }

    // Fallback: enumerate all devices (triggers ALSA errors but may find something)
    for host_id in cpal::available_hosts() {
        let host = match cpal::host_from_id(host_id) {
            Ok(h) => h,
            Err(_) => continue,
        };
        if let Ok(devices) = host.input_devices() {
            for d in devices {
                if let Ok(name) = d.name() {
                    if let Ok(config) = d.default_input_config() {
                        return Some((name, d, config));
                    }
                }
            }
        }
    }
    None
}

fn start_capture(
    device: &cpal::Device,
    config: &cpal::SupportedStreamConfig,
    audio_source: &NativeAudioSource,
    input_level: &Arc<Mutex<f32>>,
    dev_name: &str,
) -> bool {
    let sample_rate = config.sample_rate().0;
    let channels = config.channels() as u32;
    let level = input_level.clone();
    let name = dev_name.to_string();

    // Channel: cpal callback (sync) → tokio task (async for capture_frame)
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<Vec<i16>>();

    let err_fn = move |err| eprintln!("[audio] {} capture error: {}", name, err);

    let stream_res = match config.sample_format() {
        cpal::SampleFormat::F32 => {
            let tx = tx.clone();
            let level = level.clone();
            device.build_input_stream(
                &config.clone().into(),
                move |data: &[f32], _: &_| {
                    let rms = compute_rms_level_f32(data);
                    { let mut l = level.lock().unwrap(); *l = rms; }
                    let pcm16: Vec<i16> = data.iter().map(|&s| (s.clamp(-1.0, 1.0) * 32767.0) as i16).collect();
                    let _ = tx.send(pcm16);
                },
                err_fn, None,
            )
        }
        cpal::SampleFormat::I16 => {
            let tx = tx.clone();
            let level = level.clone();
            device.build_input_stream(
                &config.clone().into(),
                move |data: &[i16], _: &_| {
                    let rms = compute_rms_level_i16(data);
                    { let mut l = level.lock().unwrap(); *l = rms; }
                    let _ = tx.send(data.to_vec());
                },
                err_fn, None,
            )
        }
        cpal::SampleFormat::U16 => {
            let tx = tx.clone();
            let level = level.clone();
            device.build_input_stream(
                &config.clone().into(),
                move |data: &[u16], _: &_| {
                    let pcm16: Vec<i16> = data.iter().map(|&s| (s as i32 - 32768).clamp(-32768, 32767) as i16).collect();
                    let rms = compute_rms_level_i16(&pcm16);
                    { let mut l = level.lock().unwrap(); *l = rms; }
                    let _ = tx.send(pcm16);
                },
                err_fn, None,
            )
        }
        _ => return false,
    };

    match stream_res {
        Ok(stream) => {
            if let Err(e) = stream.play() {
                eprintln!("[audio] {} play error: {}", dev_name, e);
                return false;
            }
            eprintln!("[audio] Capture started on {}", dev_name);
            std::mem::forget(stream);

            // Spawn async task to forward frames to WebRTC
            let src = audio_source.clone();
            tokio::spawn(async move {
                while let Some(pcm16) = rx.recv().await {
                    let samples_per_channel = (pcm16.len() as u32) / channels;
                    if let Err(e) = src.capture_frame(&AudioFrame {
                        data: pcm16.into(),
                        sample_rate,
                        num_channels: channels,
                        samples_per_channel,
                    }).await {
                        eprintln!("[audio] capture_frame error: {}", e);
                    }
                }
            });
            true
        }
        Err(e) => {
            eprintln!("[audio] {} build error: {}", dev_name, e);
            false
        }
    }
}

pub fn spawn_speaker_task(
    audio_track: RemoteAudioTrack,
    output_level: Arc<Mutex<f32>>,
) {
    let handle = tokio::runtime::Handle::current();
    std::thread::spawn(move || {
        handle.block_on(async move {
            let (dev_name, device, dev_config) = match find_output_device() {
                Some(d) => d,
                None => {
                    eprintln!("[audio] No working output device found");
                    return;
                }
            };

            let sample_rate = dev_config.sample_rate().0;
            let channels = dev_config.channels();

            eprintln!("[audio] Using output device: {} ({}Hz {}ch {:?})", dev_name, sample_rate, channels, dev_config.sample_format());

            let mut native_stream = NativeAudioStream::new(
                audio_track.rtc_track(),
                sample_rate as i32,
                channels as i32,
            );

            let sample_buffer = Arc::new(Mutex::new(VecDeque::<f32>::new()));
            let err_name = dev_name.clone();
            let err_fn = move |err| eprintln!("[audio] {} output error: {}", err_name, err);

            let stream_res = match dev_config.sample_format() {
                cpal::SampleFormat::F32 => {
                    let buf_clone = sample_buffer.clone();
                    device.build_output_stream(
                        &dev_config.clone().into(),
                        move |data: &mut [f32], _: &_| {
                            let mut buf = buf_clone.lock().unwrap();
                            for sample in data.iter_mut() {
                                *sample = buf.pop_front().unwrap_or(0.0);
                            }
                        },
                        err_fn, None,
                    )
                }
                cpal::SampleFormat::I16 => {
                    let buf_clone = sample_buffer.clone();
                    device.build_output_stream(
                        &dev_config.clone().into(),
                        move |data: &mut [i16], _: &_| {
                            let mut buf = buf_clone.lock().unwrap();
                            for sample in data.iter_mut() {
                                let float_sample = buf.pop_front().unwrap_or(0.0);
                                *sample = (float_sample.clamp(-1.0, 1.0) * 32767.0) as i16;
                            }
                        },
                        err_fn, None,
                    )
                }
                cpal::SampleFormat::U16 => {
                    let buf_clone = sample_buffer.clone();
                    device.build_output_stream(
                        &dev_config.clone().into(),
                        move |data: &mut [u16], _: &_| {
                            let mut buf = buf_clone.lock().unwrap();
                            for sample in data.iter_mut() {
                                let float_sample = buf.pop_front().unwrap_or(0.0);
                                *sample = ((float_sample.clamp(-1.0, 1.0) + 1.0) * 32767.5) as u16;
                            }
                        },
                        err_fn, None,
                    )
                }
                _ => {
                    eprintln!("[audio] Unsupported output format on {}", dev_name);
                    return;
                }
            };

            match stream_res {
                Ok(stream) => {
                    if let Err(e) = stream.play() {
                        eprintln!("[audio] {} play error: {}", dev_name, e);
                        return;
                    }
                    eprintln!("[audio] Playback started on {}", dev_name);
                    while let Some(frame) = native_stream.next().await {
                        let rms = compute_rms_level_i16(&frame.data);
                        { let mut l = output_level.lock().unwrap(); *l = rms; }
                        let mut buf = sample_buffer.lock().unwrap();
                        for &s in frame.data.iter() {
                            buf.push_back((s as f32) / 32768.0);
                        }
                    }
                }
                Err(e) => {
                    eprintln!("[audio] {} build error: {}", dev_name, e);
                }
            }
        });
    });
}

fn find_output_device() -> Option<(String, cpal::Device, cpal::SupportedStreamConfig)> {
    for host_id in cpal::available_hosts() {
        let host = match cpal::host_from_id(host_id) {
            Ok(h) => h,
            Err(_) => continue,
        };

        for preferred in &["default", "pipewire", "sysdefault"] {
            if let Ok(devices) = host.output_devices() {
                for d in devices {
                    if let Ok(name) = d.name() {
                        if name == *preferred {
                            if let Ok(config) = d.default_output_config() {
                                return Some((name, d, config));
                            }
                        }
                    }
                }
            }
        }
    }

    for host_id in cpal::available_hosts() {
        let host = match cpal::host_from_id(host_id) {
            Ok(h) => h,
            Err(_) => continue,
        };
        if let Ok(devices) = host.output_devices() {
            for d in devices {
                if let Ok(name) = d.name() {
                    if let Ok(config) = d.default_output_config() {
                        return Some((name, d, config));
                    }
                }
            }
        }
    }
    None
}
