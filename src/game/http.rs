use std::net::SocketAddr;

use axum::{
    Json,
    extract::{ConnectInfo, Path, Request, WebSocketUpgrade},
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse as _, Response},
};

use crate::{
    openapi::Null,
    types::{
        RoomId,
        http::{CreateRoom, HttpRoomConfig, RoomData, RoomList, RoomSummary},
    },
    util::{JsonResponse, WebSocketAction, log_ws},
};

use super::{
    MINIMUM_SERVER_VERSION,
    session::{
        GameSession,
        map::{GAME_SESSION_MAP, GameSessionLock},
        room::config::RoomConfig,
    },
    ws::handle_socket,
};

/// クライアントに要求する最小のプロトコルバージョンを返す
#[utoipa::path(
    get,
    path = "/version",
    tag = "server",
    responses(
        (
            status = OK,
            description = "クライアントに要求する最小のプロトコルバージョン",
            body = usize,
            content_type = "text/plain",
        ),
    ),
)]
pub async fn version() -> Response {
    MINIMUM_SERVER_VERSION.to_string().into_response()
}

/// 新しい部屋を作る
#[utoipa::path(
    post,
    path = "/new",
    tag = "rooms",
    request_body = HttpRoomConfig,
    responses(
        (status = CREATED, description = "部屋を作成した", body = CreateRoom),
        (status = BAD_REQUEST, description = "設定が不正", body = CreateRoom),
        (
            status = UNSUPPORTED_MEDIA_TYPE,
            description = "Content-Type が application/json ではない",
        ),
        (
            status = UNPROCESSABLE_ENTITY,
            description = "ボディを JSON として解釈できない",
        ),
    ),
)]
pub async fn new_room(Json(config): Json<HttpRoomConfig>) -> Response {
    let config = match RoomConfig::try_from(config) {
        Ok(config) => config,
        Err(message) => {
            return JsonResponse {
                status_code: StatusCode::BAD_REQUEST,
                content: CreateRoom::Err { message },
            }
            .into_response();
        }
    };
    let room_id = RoomId::new();
    GAME_SESSION_MAP
        .write()
        .insert(room_id, GameSession::new(config));
    JsonResponse {
        status_code: StatusCode::CREATED,
        content: CreateRoom::Ok { room_id },
    }
    .into_response()
}

/// 存在する部屋を全て返す
#[utoipa::path(
    get,
    path = "/",
    tag = "rooms",
    responses((status = OK, description = "存在する部屋の一覧", body = RoomList)),
)]
pub async fn room_list() -> Response {
    let ids: Vec<RoomId> = GAME_SESSION_MAP.read().keys().copied().collect();
    JsonResponse {
        status_code: StatusCode::OK,
        content: RoomList(
            ids.into_iter()
                .map(|id| RoomSummary {
                    id,
                    players: GameSessionLock::new(id).with_read(|session| {
                        session
                            .get_room()
                            .get_players()
                            .iter()
                            .map(|player| player.name.to_owned())
                            .collect()
                    }),
                })
                .collect(),
        ),
    }
    .into_response()
}

pub async fn room_existence_check(
    Path(room_id): Path<RoomId>,
    req: Request,
    next: Next,
) -> Response {
    if GAME_SESSION_MAP.read().get(&room_id).is_none() {
        return JsonResponse {
            status_code: StatusCode::NOT_FOUND,
            content: None::<()>,
        }
        .into_response();
    }
    next.run(req).await
}

// vvv 部屋が存在するものとして処理を進めてOK vvv

/// 部屋の現在の状態を返す
#[utoipa::path(
    get,
    path = "/",
    tag = "rooms",
    params(("room_id" = RoomId, Path)),
    responses(
        (status = OK, description = "部屋の現在の状態", body = RoomData),
        (status = NOT_FOUND, description = "部屋が存在しない", body = Null),
    ),
)]
pub async fn room_info(Path(room_id): Path<RoomId>) -> Response {
    GameSessionLock::new(room_id).with_read(|session| {
        let room = session.get_room();
        JsonResponse {
            status_code: StatusCode::OK,
            content: RoomData {
                room_id,
                current_turn: room.get_current_turn(),
                players: room.get_players(),
                pieces: room.get_pieces(),
            },
        }
        .into_response()
    })
}

/// 対局用の WebSocket に繋ぐ
///
/// 接続後にやり取りするメッセージは `PlayerAction` / `Responses` / `RegisterRoomEvent` を参照
#[utoipa::path(
    get,
    path = "/ws",
    tag = "websocket",
    params(("room_id" = RoomId, Path)),
    responses(
        (status = SWITCHING_PROTOCOLS, description = "WebSocket に切り替えた"),
        (
            status = BAD_REQUEST,
            description = "WebSocket へのアップグレードを要求していない",
        ),
        (status = NOT_FOUND, description = "部屋が存在しない", body = Null),
    ),
)]
pub async fn serve_ws(
    Path(room_id): Path<RoomId>,
    ws: WebSocketUpgrade,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
) -> Response {
    ws.on_upgrade(move |socket| async move {
        let ip = addr.ip();
        log_ws(ip, WebSocketAction::Connect);
        handle_socket(socket, ip, room_id).await;
        log_ws(ip, WebSocketAction::Disconnect);
    })
}
