use livekit::prelude::Room;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

#[derive(Clone, Debug, PartialEq)]
pub enum AppScreen {
    Login,
    ContactList,
    Settings,
    Ringing { caller: String },
    Calling { target: String },
    JoinRoom,
    InCall,
    RoomBrowser,
    /// room_name: 対象ルーム名, invited_users: 選択済みユーザーのSet
    InviteRoom {
        room_name: String,
        invited_users: Vec<String>,
    },
    Error(String),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum RenderMode {
    Braille,   // Odin (pixel animation)
    HalfBlock, // Zig (mosaic)
}

#[derive(Clone, Debug)]
pub struct AnnouncedRoom {
    pub owner: String,
    pub name: String,
}

pub struct AppState {
    pub screen: AppScreen,
    pub username: String,
    pub input_buffer: String,
    // Form fields for Login
    pub livekit_url: String,
    pub api_key: String,
    pub api_secret: String,
    pub active_input_index: usize, // 0: Username, 1: URL, 2: API Key, 3: Secret

    pub selected_index: usize,
    pub users: Vec<String>,
    pub livekit_lobby: Option<Room>,   // Persistent lobby room for presence & signaling
    pub livekit_room: Option<Room>,    // Active call room
    pub is_muted: bool,
    pub room_name: String,
    pub room_is_public: bool,
    pub status_messages: Arc<Mutex<Vec<(String, StatusKind)>>>,
    pub participant_quality: Arc<Mutex<HashMap<String, u8>>>,
    pub announced_rooms: Arc<Mutex<Vec<AnnouncedRoom>>>,
    pub remote_video_frames: Arc<Mutex<HashMap<String, (Vec<u8>, u32, u32)>>>,
    pub local_video_frame: Arc<Mutex<Option<(Vec<u8>, u32, u32)>>>,
    pub render_mode: RenderMode,
    pub audio_input_level: Arc<Mutex<f32>>,
    pub audio_output_level: Arc<Mutex<f32>>,
    /// 最後に切断したピアの名前（InCall 中の通知用）
    pub disconnected_peer: Arc<Mutex<Option<String>>>,
}

/// ステータスメッセージの種別（色分け用）
#[derive(Clone, Debug, PartialEq)]
pub enum StatusKind {
    Join,       // 緑: 参加
    Leave,      // 赤: 退室・切断
    Info,       // 黄: その他
}

impl AppState {
    pub fn new() -> Self {
        let cfg = crate::config::load();
        Self {
            screen: AppScreen::Login,
            username: String::new(),
            input_buffer: cfg.last_username.clone(),
            livekit_url: cfg.livekit_url,
            api_key: cfg.api_key,
            api_secret: cfg.api_secret,
            active_input_index: 0,
            selected_index: 0,
            users: Vec::new(),
            livekit_lobby: None,
            livekit_room: None,
            is_muted: false,
            room_name: String::new(),
            room_is_public: true,
            status_messages: Arc::new(Mutex::new(Vec::new())),
            participant_quality: Arc::new(Mutex::new(HashMap::new())),
            announced_rooms: Arc::new(Mutex::new(Vec::new())),
            remote_video_frames: Arc::new(Mutex::new(HashMap::new())),
            local_video_frame: Arc::new(Mutex::new(None)),
            audio_input_level: Arc::new(Mutex::new(0.0)),
            audio_output_level: Arc::new(Mutex::new(0.0)),
            disconnected_peer: Arc::new(Mutex::new(None)),
            render_mode: match cfg.render_mode.as_deref() {
                Some("halfblock") => RenderMode::HalfBlock,
                _ => RenderMode::Braille,
            },
        }
    }

    pub fn push_status(&self, msg: String, kind: StatusKind) {
        let mut msgs = self.status_messages.lock().unwrap();
        msgs.push((msg, kind));
        if msgs.len() > 20 {
            msgs.remove(0);
        }
    }
}
