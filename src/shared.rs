use serde::{Deserialize, Serialize};

/// Signaling messages sent over LiveKit Data Channel between clients.
/// These replace the previous server-relayed ClientMessage / ServerMessage.
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "type")]
pub enum SignalingMessage {
    /// Caller → Callee: "I want to call you"
    CallRequest { from: String, to: String },
    /// Callee → Caller: "I accepted, here is the room to join"
    CallAccepted { from: String, to: String, room: String },
    /// Callee → Caller: "I rejected your call"
    CallRejected { from: String, to: String },
    /// Either side: "Hang up"
    CallEnded { from: String, to: String },
}
