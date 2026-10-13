use std::{net::IpAddr, sync::Arc};

use axum::extract::ws::{CloseFrame, Message, Utf8Bytes, WebSocket, close_code};
use futures_util::{SinkExt as _, StreamExt as _};
use parking_lot::Mutex;
use tokio::sync::mpsc;
use uuid::Uuid;

use crate::{
    types::{
        RoomId,
        ws::{
            ActionNotAcceptedReason, CreateUser, CreateUserError, InitialRoomState, PlayerAction,
            PlayerActionWithAuth, PlayerActionWithoutAuth, Responses,
        },
    },
    util::{WebSocketReceiveAction, WebSocketSendAction, generate_name, log_error, log_ws},
};

use super::{QUEUE_MESSAGE_LIMIT, session::map::GameSessionLock};

#[derive(Default, Clone, Copy)]
struct ConnectionPlayer {
    id: Option<Uuid>,
    authorized: bool,
}

impl ConnectionPlayer {
    fn authorized_id(&self) -> Option<Uuid> {
        self.id.filter(|_| self.authorized)
    }
}

#[inline(always)]
pub async fn handle_socket(socket: WebSocket, ip: IpAddr, room_id: RoomId) {
    let session_lock = GameSessionLock::new(room_id);
    let (mut queue_rx, initial_room_state) = session_lock.with_read(|session| {
        let room_data = session.get_room().to_data(room_id);
        (
            session.subscribe_queue(),
            serde_json::to_string(&InitialRoomState(room_data)).unwrap(),
        )
    });
    let (conn_tx, mut conn_rx) = mpsc::channel(QUEUE_MESSAGE_LIMIT);
    let (mut sender, mut receiver) = socket.split();

    let mut send_task = tokio::spawn(async move {
        macro_rules! send_text_message {
            ($text:expr) => {{
                let res = sender.send(Message::Text($text.to_owned().into())).await;
                match res {
                    Ok(_) => log_ws(ip, Ok(WebSocketSendAction::SendText($text))),
                    Err(_) => log_ws(ip, Err(WebSocketSendAction::SendText($text))),
                }
                res
            }};
        }
        macro_rules! send_close_message {
            ($code:expr, $reason:literal) => {{
                let cf = CloseFrame {
                    code: $code,
                    reason: Utf8Bytes::from_static($reason),
                };
                match sender.send(Message::Close(Some(cf.clone()))).await {
                    Ok(_) => log_ws(ip, Ok(WebSocketSendAction::SendClose(cf))),
                    Err(_) => log_ws(ip, Err(WebSocketSendAction::SendClose(cf))),
                }
            }};
        }
        if send_text_message!(&initial_room_state).is_err() {
            return;
        }
        loop {
            tokio::select! {
                val = queue_rx.recv() => match val {
                    Ok(event) => {
                        let event_str = serde_json::to_string(&event).unwrap();
                        if send_text_message!(&event_str).is_err() {
                            break;
                        }
                    }
                    Err(error) => {
                        log_error!("queue_recv", error);
                        send_close_message!(close_code::AGAIN, "Server Lagged");
                        break;
                    }
                },
                val = conn_rx.recv() => match val {
                    Some(
                        response @ (Responses::UserCreated(_)
                        | Responses::Authorized
                        | Responses::ActionNotAccepted(_)
                        | Responses::SessionExpired),
                    ) => {
                        let text = serde_json::to_string(&response).unwrap();
                        if send_text_message!(&text).is_err() {
                            break;
                        }
                    }
                    Some(Responses::AuthorizedInternal(_)) => unreachable!(),
                    Some(Responses::GotInvalidData) => {
                        send_close_message!(close_code::INVALID, "Invalid Data");
                        break;
                    }
                    None => break,
                },
            }
        }
    });

    let player: Arc<Mutex<ConnectionPlayer>> = Arc::new(Mutex::new(ConnectionPlayer::default()));
    let mut recv_task = tokio::spawn({
        let player = Arc::clone(&player);
        async move {
            while let Some(Ok(msg)) = receiver.next().await {
                let Ok(permit) = conn_tx.reserve().await else {
                    break;
                };
                match msg {
                    Message::Text(text) => {
                        log_ws(ip, WebSocketReceiveAction::GotText(&text));
                        if let Ok(action) = serde_json::from_str::<PlayerAction>(&text) {
                            let authorized_id = player.lock().authorized_id();
                            if let Some(msg) = handle_game(action, room_id, authorized_id) {
                                match msg {
                                    Responses::AuthorizedInternal(authorized_private_id) => {
                                        *player.lock() = ConnectionPlayer {
                                            id: Some(authorized_private_id),
                                            authorized: true,
                                        };
                                        permit.send(Responses::Authorized);
                                    }
                                    Responses::UserCreated(CreateUser::Ok {
                                        private_id, ..
                                    }) => {
                                        player.lock().id = Some(private_id);
                                        permit.send(msg);
                                    }
                                    Responses::SessionExpired => {
                                        player.lock().authorized = false;
                                        permit.send(msg);
                                    }
                                    _ => permit.send(msg),
                                }
                            }
                        } else {
                            permit.send(Responses::GotInvalidData);
                        }
                    }
                    Message::Close(c) => {
                        log_ws(ip, WebSocketReceiveAction::GotClose(c));
                        break;
                    }
                    Message::Ping(_) | Message::Pong(_) => {}
                    _ => permit.send(Responses::GotInvalidData),
                }
            }
        }
    });

    tokio::select! {
        _ = (&mut send_task) => recv_task.abort(),
        _ = (&mut recv_task) => send_task.abort(),
    }
    if let Some(private_id) = player.lock().id {
        session_lock.with_write(|session| session.get_room_mut().mark_disconnected(private_id));
    };
}

#[inline(always)]
fn handle_game(
    action: PlayerAction,
    room_id: RoomId,
    private_id: Option<Uuid>,
) -> Option<Responses> {
    GameSessionLock::new(room_id).with_write(|session| {
        let room = session.get_room_mut();
        room.evict_expired_players();
        match action {
            PlayerAction::WithoutAuth(action) => match action {
                PlayerActionWithoutAuth::JoinTeam(side) => {
                    match room.create_player(side, generate_name()) {
                        Some((private_id, public_id)) => {
                            return Some(Responses::UserCreated(CreateUser::Ok {
                                private_id,
                                public_id,
                            }));
                        }
                        None => {
                            return Some(Responses::UserCreated(CreateUser::Err {
                                message: CreateUserError::PlayerLimitExceeded,
                            }));
                        }
                    }
                }
                PlayerActionWithoutAuth::Resume(private_id) => {
                    if !room.contains_player(private_id) {
                        return Some(Responses::SessionExpired);
                    }
                    room.mark_connected(private_id);
                    return Some(Responses::AuthorizedInternal(private_id));
                }
            },
            PlayerAction::WithAuth(action) => match private_id {
                Some(private_id) => {
                    if !room.contains_player(private_id) {
                        return Some(Responses::SessionExpired);
                    }
                    match action {
                        PlayerActionWithAuth::LeaveTeam => {
                            room.remove_player(private_id);
                        }
                        PlayerActionWithAuth::Move(old_position, new_position) => {
                            if let Err(error) =
                                room.move_piece(private_id, old_position, new_position)
                            {
                                return Some(Responses::ActionNotAccepted(error.into()));
                            }
                        }
                    }
                }
                None => {
                    return Some(Responses::ActionNotAccepted(
                        ActionNotAcceptedReason::Unauthorized,
                    ));
                }
            },
        }
        None
    })
}
