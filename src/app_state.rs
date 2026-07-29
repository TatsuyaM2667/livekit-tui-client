use livekit::prelude::Room;
use std::sync::{Arc, Mutex};

#[derive(Clone, Debug, PartialEq)]
pub enum AppScreen {
    Login,
    ContactList,
    Ringing { caller: String },
    Calling { target: String },
    InCall,
    Error(String),
}

#[derive(Clone, Debug, PartialEq)]
pub enum RenderMode {
    Braille,   // Odin (pixel animation)
    HalfBlock, // Zig (mosaic)
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
    pub remote_video_frame: Arc<Mutex<Option<(Vec<u8>, u32, u32)>>>,
    pub render_mode: RenderMode,
}

impl AppState {
    pub fn new() -> Self {
        dotenvy::dotenv().ok();
        Self {
            screen: AppScreen::Login,
            username: String::new(),
            input_buffer: String::new(),
            livekit_url: std::env::var("LIVEKIT_URL").unwrap_or_else(|_| "wss://your-project.livekit.cloud".to_string()),
            api_key: std::env::var("LIVEKIT_API_KEY").unwrap_or_default(),
            api_secret: std::env::var("LIVEKIT_API_SECRET").unwrap_or_default(),
            active_input_index: 0,
            selected_index: 0,
            users: Vec::new(),
            livekit_lobby: None,
            livekit_room: None,
            is_muted: false,
            remote_video_frame: Arc::new(Mutex::new(None)),
            render_mode: RenderMode::Braille,
        }
    }
}

