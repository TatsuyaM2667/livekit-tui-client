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

pub struct AppState {
    pub screen: AppScreen,
    pub username: String,
    pub input_buffer: String,
    pub selected_index: usize,
    pub users: Vec<String>,
    pub livekit_lobby: Option<Room>,   // Persistent lobby room for presence & signaling
    pub livekit_room: Option<Room>,    // Active call room
    pub is_muted: bool,
    pub remote_video_frame: Arc<Mutex<Option<(Vec<u8>, u32, u32)>>>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            screen: AppScreen::Login,
            username: String::new(),
            input_buffer: String::new(),
            selected_index: 0,
            users: Vec::new(),
            livekit_lobby: None,
            livekit_room: None,
            is_muted: false,
            remote_video_frame: Arc::new(Mutex::new(None)),
        }
    }
}
