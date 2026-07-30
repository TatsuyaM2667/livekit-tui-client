use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "type")]
pub enum SignalingMessage {
    /// Caller → Callee: "I want to call you"
    CallRequest { from: String, to: String },
    /// Callee → Caller: "I accepted, here is the room to join"
    CallAccepted {
        from: String,
        to: String,
        room: String,
    },
    /// Callee → Caller: "I rejected your call"
    CallRejected { from: String, to: String },
    /// Either side: "Hang up"
    CallEnded { from: String, to: String },
    /// Room owner announces a public room to lobby
    RoomAnnounce {
        from: String,
        room: String,
    },
    /// Room owner removes a public room listing
    RoomRemove {
        from: String,
        room: String,
    },
    /// Invite a specific user to join a room
    RoomInvite {
        from: String,
        to: String,
        room: String,
    },
}
