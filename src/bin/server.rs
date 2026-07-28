use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    response::IntoResponse,
    routing::get,
    Router,
};
use futures_util::{SinkExt, StreamExt};
use jsonwebtoken::{encode, Algorithm, EncodingKey, Header};
use livekit_tui_client::shared::{ClientMessage, ServerMessage};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::env;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::{mpsc, Mutex};

type Tx = mpsc::UnboundedSender<Message>;

struct AppState {
    users: Mutex<HashMap<String, Tx>>,
}

#[derive(Serialize, Deserialize)]
#[allow(non_snake_case)]
struct VideoGrants {
    roomJoin: bool,
    room: String,
}

#[derive(Serialize, Deserialize)]
struct Claims {
    iss: String,
    sub: String,
    name: String,
    video: VideoGrants,
    exp: usize,
    nbf: usize,
}

fn create_livekit_token(api_key: &str, api_secret: &str, identity: &str, room: &str) -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as usize;

    let claims = Claims {
        iss: api_key.to_string(),
        sub: identity.to_string(),
        name: identity.to_string(),
        video: VideoGrants {
            roomJoin: true,
            room: room.to_string(),
        },
        exp: now + 3600, // 1 hour expiration
        nbf: now,
    };

    let mut header = Header::new(Algorithm::HS256);
    header.typ = Some("JWT".to_string());

    encode(
        &header,
        &claims,
        &EncodingKey::from_secret(api_secret.as_bytes()),
    )
    .unwrap_or_default()
}

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();
    println!("Starting signaling server on ws://0.0.0.0:3000/ws");

    let state = Arc::new(AppState {
        users: Mutex::new(HashMap::new()),
    });

    let app = Router::new()
        .route("/ws", get(ws_handler))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

async fn ws_handler(ws: WebSocketUpgrade, State(state): State<Arc<AppState>>) -> impl IntoResponse {
    ws.on_upgrade(|socket| handle_socket(socket, state))
}

async fn handle_socket(socket: WebSocket, state: Arc<AppState>) {
    let (mut sender, mut receiver) = socket.split();
    let (tx, mut rx) = mpsc::unbounded_channel();
    let mut current_user: Option<String> = None;

    tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            if sender.send(msg).await.is_err() {
                break;
            }
        }
    });

    while let Some(Ok(msg)) = receiver.next().await {
        if let Message::Text(text) = msg {
            if let Ok(client_msg) = serde_json::from_str::<ClientMessage>(&text) {
                match client_msg {
                    ClientMessage::Login { username } => {
                        let mut users = state.users.lock().await;
                        if users.contains_key(&username) {
                            let _ = tx.send(Message::Text(
                                serde_json::to_string(&ServerMessage::Error {
                                    message: "Username already taken".to_string(),
                                })
                                .unwrap(),
                            ));
                        } else {
                            users.insert(username.clone(), tx.clone());
                            current_user = Some(username.clone());
                            let _ = tx.send(Message::Text(
                                serde_json::to_string(&ServerMessage::LoginSuccess {
                                    username: username.clone(),
                                })
                                .unwrap(),
                            ));
                            broadcast_user_list(&users);
                        }
                    }
                    ClientMessage::CallRequest { target_username } => {
                        if let Some(ref me) = current_user {
                            let users = state.users.lock().await;
                            if let Some(target_tx) = users.get(&target_username) {
                                let _ = target_tx.send(Message::Text(
                                    serde_json::to_string(&ServerMessage::IncomingCall {
                                        from_username: me.clone(),
                                    })
                                    .unwrap(),
                                ));
                            } else {
                                let _ = tx.send(Message::Text(
                                    serde_json::to_string(&ServerMessage::Error {
                                        message: "Target user offline".to_string(),
                                    })
                                    .unwrap(),
                                ));
                            }
                        }
                    }
                    ClientMessage::CallAccept { caller_username } => {
                        if let Some(ref me) = current_user {
                            let users = state.users.lock().await;
                            if let Some(caller_tx) = users.get(&caller_username) {
                                let room_name = format!("room_{}_{}", caller_username, me);
                                
                                let api_key = env::var("LIVEKIT_API_KEY").unwrap_or_default();
                                let api_secret = env::var("LIVEKIT_API_SECRET").unwrap_or_default();

                                let caller_token = create_livekit_token(&api_key, &api_secret, &caller_username, &room_name);
                                let my_token = create_livekit_token(&api_key, &api_secret, me, &room_name);

                                let _ = caller_tx.send(Message::Text(
                                    serde_json::to_string(&ServerMessage::CallAccepted {
                                        room_name: room_name.clone(),
                                        token: caller_token,
                                    })
                                    .unwrap(),
                                ));

                                let _ = tx.send(Message::Text(
                                    serde_json::to_string(&ServerMessage::CallAccepted {
                                        room_name,
                                        token: my_token,
                                    })
                                    .unwrap(),
                                ));
                            }
                        }
                    }
                    ClientMessage::CallReject { caller_username } => {
                        if let Some(ref me) = current_user {
                            let users = state.users.lock().await;
                            if let Some(caller_tx) = users.get(&caller_username) {
                                let _ = caller_tx.send(Message::Text(
                                    serde_json::to_string(&ServerMessage::CallRejected {
                                        target_username: me.clone(),
                                    })
                                    .unwrap(),
                                ));
                            }
                        }
                    }
                }
            }
        }
    }

    if let Some(username) = current_user {
        let mut users = state.users.lock().await;
        users.remove(&username);
        broadcast_user_list(&users);
    }
}

fn broadcast_user_list(users: &HashMap<String, Tx>) {
    let list: Vec<String> = users.keys().cloned().collect();
    let msg = Message::Text(serde_json::to_string(&ServerMessage::UserList { users: list }).unwrap());
    for tx in users.values() {
        let _ = tx.send(msg.clone());
    }
}
