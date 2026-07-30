use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use futures_util::StreamExt;
use livekit::options::TrackPublishOptions;
use livekit::prelude::*;
use livekit::webrtc::audio_source::native::NativeAudioSource;
use livekit::webrtc::audio_stream::native::NativeAudioStream;
use livekit::webrtc::prelude::*;
use std::collections::VecDeque;
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

        if let Ok(devices) = host.input_devices() {
            for d in devices {
                let name = d.name().unwrap_or_default();
                if let Ok(cfg) = d.default_input_config() {
                    eprintln!("[audio]     Input: {} ({:?} {}Hz {}ch)", name, cfg.sample_format(), cfg.sample_rate().0, cfg.channels());
                } else {
                    eprintln!("[audio]     Input: {} (no default config)", name);
                }
            }
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

        if let Ok(devices) = host.output_devices() {
            for d in devices {
                let name = d.name().unwrap_or_default();
                if let Ok(cfg) = d.default_output_config() {
                    eprintln!("[audio]     Output: {} ({:?} {}Hz {}ch)", name, cfg.sample_format(), cfg.sample_rate().0, cfg.channels());
                } else {
                    eprintln!("[audio]     Output: {} (no default config)", name);
                }
            }
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

    let audio_src = audio_source.clone();
    let mut started = false;

    'outer: for host_id in cpal::available_hosts() {
        let host = match cpal::host_from_id(host_id) {
            Ok(h) => h,
            Err(_) => continue,
        };

        let devices: Vec<_> = match host.input_devices() {
            Ok(d) => d.filter_map(|d| {
                let name = d.name().ok();
                let config = d.default_input_config().ok();
                (name.is_some() && config.is_some()).then(|| (name.unwrap(), d, config.unwrap()))
            }).collect(),
            Err(_) => continue,
        };

        if devices.is_empty() {
            // Fall back to default device
            if let Some(d) = host.default_input_device() {
                if let Ok(c) = d.default_input_config() {
                    let name = d.name().unwrap_or_default();
                    let devices_fallback = vec![(name, d, c)];
                    if try_capture_device(&devices_fallback, &audio_src, &input_level, host_id) {
                        started = true;
                        break 'outer;
                    }
                }
            }
            continue;
        }

        if try_capture_device(&devices, &audio_src, &input_level, host_id) {
            started = true;
            break;
        }
    }

    if !started {
        eprintln!("[audio] No working capture device found on any host");
    }

    Some(publication)
}

fn try_capture_device(
    devices: &[(String, cpal::Device, cpal::SupportedStreamConfig)],
    audio_src: &NativeAudioSource,
    input_level: &Arc<Mutex<f32>>,
    host_id: cpal::HostId,
) -> bool {
    for (name, device, config) in devices {
        eprintln!("[audio] Trying capture on {} ({:?}) ...", name, host_id);

        let sample_rate = config.sample_rate().0;
        let channels = config.channels() as u32;
        let src = audio_src.clone();
        let level = input_level.clone();
        let dev_name = name.clone();
        let err_fn = move |err| eprintln!("[audio] {} capture error: {}", dev_name, err);

        let stream_res = match config.sample_format() {
            cpal::SampleFormat::F32 => {
                let level = level.clone();
                device.build_input_stream(
                    &config.clone().into(),
                    move |data: &[f32], _: &_| {
                        let rms = compute_rms_level_f32(data);
                        {
                            let mut l = level.lock().unwrap();
                            *l = rms;
                        }
                        let pcm16: Vec<i16> = data.iter().map(|&s| (s.clamp(-1.0, 1.0) * 32767.0) as i16).collect();
                        let samples_per_channel = (pcm16.len() as u32) / channels;
                        let _ = src.capture_frame(&AudioFrame {
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
                let level = level.clone();
                device.build_input_stream(
                    &config.clone().into(),
                    move |data: &[i16], _: &_| {
                        let rms = compute_rms_level_i16(data);
                        {
                            let mut l = level.lock().unwrap();
                            *l = rms;
                        }
                        let samples_per_channel = (data.len() as u32) / channels;
                        let _ = src.capture_frame(&AudioFrame {
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
                let level = level.clone();
                device.build_input_stream(
                    &config.clone().into(),
                    move |data: &[u16], _: &_| {
                        let pcm16: Vec<i16> = data.iter().map(|&s| (s as i32 - 32768).clamp(-32768, 32767) as i16).collect();
                        let rms = compute_rms_level_i16(&pcm16);
                        {
                            let mut l = level.lock().unwrap();
                            *l = rms;
                        }
                        let samples_per_channel = (pcm16.len() as u32) / channels;
                        let _ = src.capture_frame(&AudioFrame {
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
            _ => continue,
        };

        match stream_res {
            Ok(stream) => {
                if let Err(e) = stream.play() {
                    eprintln!("[audio] {} play error: {}", name, e);
                    continue;
                }
                eprintln!("[audio] Capture started on {}", name);
                std::mem::forget(stream);
                return true;
            }
            Err(e) => {
                eprintln!("[audio] {} build error: {}", name, e);
            }
        }
    }
    false
}

pub fn spawn_speaker_task(
    audio_track: RemoteAudioTrack,
    output_level: Arc<Mutex<f32>>,
) {
    let handle = tokio::runtime::Handle::current();
    std::thread::spawn(move || {
        handle.block_on(async move {
            let level_ref = output_level.clone();

            for host_id in cpal::available_hosts() {
                let host = match cpal::host_from_id(host_id) {
                    Ok(h) => h,
                    Err(_) => continue,
                };

                let devices: Vec<(String, cpal::Device, cpal::SupportedStreamConfig)> = match host.output_devices() {
                    Ok(d) => d.filter_map(|d| {
                        let name = d.name().ok();
                        let config = d.default_output_config().ok();
                        (name.is_some() && config.is_some()).then(|| (name.unwrap(), d, config.unwrap()))
                    }).collect(),
                    Err(_) => {
                        // Fall back to default device
                        if let Some(d) = host.default_output_device() {
                            if let Ok(c) = d.default_output_config() {
                                vec![(d.name().unwrap_or_default(), d, c)]
                            } else {
                                continue;
                            }
                        } else {
                            continue;
                        }
                    }
                };

                for (name, device, config) in &devices {
                    eprintln!("[audio] Trying playback on {} ({:?}) ...", name, host_id);

                    let sample_rate = config.sample_rate().0;
                    let channels = config.channels();

                    let mut native_stream = NativeAudioStream::new(
                        audio_track.rtc_track(),
                        sample_rate as i32,
                        channels as i32,
                    );

                    let sample_buffer = Arc::new(Mutex::new(VecDeque::<f32>::new()));
                    let dev_name = name.clone();
                    let err_fn = move |err| eprintln!("[audio] {} output error: {}", dev_name, err);

                    let stream_res = match config.sample_format() {
                        cpal::SampleFormat::F32 => {
                            let buf_clone = sample_buffer.clone();
                            device.build_output_stream(
                                &config.clone().into(),
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
                                &config.clone().into(),
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
                                &config.clone().into(),
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
                        _ => continue,
                    };

                    match stream_res {
                        Ok(stream) => {
                            if let Err(e) = stream.play() {
                                eprintln!("[audio] {} play error: {}, trying next device", name, e);
                                continue;
                            }
                            eprintln!("[audio] Playback started on {}", name);
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
                            return;
                        }
                        Err(e) => {
                            eprintln!("[audio] {} build error: {}, trying next device", name, e);
                        }
                    }
                }
            }

            eprintln!("[audio] No working output device found on any host");
        });
    });
}
