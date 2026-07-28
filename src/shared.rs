use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum ClientMessage {
    Login { username: String },
    CallRequest { target_username: String },
    CallAccept { caller_username: String },
    CallReject { caller_username: String },
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum ServerMessage {
    LoginSuccess {
        username: String,
    },
    UserList {
        users: Vec<String>,
    },
    IncomingCall {
        from_username: String,
    },
    CallAccepted {
        room_name: String,
        token: String,
    },
    CallRejected {
        target_username: String,
    },
    Error {
        message: String,
    },
}
