use livekit::webrtc::prelude::I420Buffer;

/// Helper: Converts RGBA image bytes to YUV420 (I420) for WebRTC video frames
pub fn rgba_to_i420(rgba: &[u8], width: u32, height: u32) -> I420Buffer {
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

/// Helper: Converts I420 WebRTC video frames to RGB bytes for rendering
pub fn i420_to_rgb(i420: &I420Buffer, width: u32, height: u32) -> Vec<u8> {
    let mut rgb = vec![0u8; (width * height * 3) as usize];
    let (stride_y, stride_u, stride_v) = i420.strides();
    let (data_y, data_u, data_v) = i420.data();

    let w = width as usize;
    let h = height as usize;

    for y in 0..h {
        for x in 0..w {
            let y_idx = y * (stride_y as usize) + x;
            let uv_x = x / 2;
            let uv_y = y / 2;
            let u_idx = uv_y * (stride_u as usize) + uv_x;
            let v_idx = uv_y * (stride_v as usize) + uv_x;

            let y_val = data_y[y_idx] as f32 - 16.0;
            let u_val = data_u[u_idx] as f32 - 128.0;
            let v_val = data_v[v_idx] as f32 - 128.0;

            let r = (1.164 * y_val + 1.596 * v_val).clamp(0.0, 255.0) as u8;
            let g = (1.164 * y_val - 0.392 * u_val - 0.813 * v_val).clamp(0.0, 255.0) as u8;
            let b = (1.164 * y_val + 2.017 * u_val).clamp(0.0, 255.0) as u8;

            let out_idx = (y * w + x) * 3;
            rgb[out_idx] = r;
            rgb[out_idx + 1] = g;
            rgb[out_idx + 2] = b;
        }
    }
    rgb
}
