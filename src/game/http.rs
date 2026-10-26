use std::net::SocketAddr;

use axum::{
    Json,
    extract::{
        ConnectInfo, Path, Query, Request, WebSocketUpgrade,
        rejection::{JsonRejection, PathRejection, QueryRejection},
    },
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse as _, Response},
};

use crate::{
    openapi::Null,
    types::{
        RoomConfig, RoomId,
        http::{
            CreateRoom, CreateRoomError, HttpRoomConfig, RoomData, RoomList, RoomSummary, WsParams,
        },
    },
    util::{JsonResponse, WebSocketAction, log_ws},
};

use super::{
    ALLOWED_CLIENT_VERSIONS,
    session::{
        GameSession,
        map::{GAME_SESSION_MAP, GameSessionLock},
    },
    ws::handle_socket,
};

/// 接続を許可するクライアントのプロトコルバージョンを返す
#[utoipa::path(
    get,
    path = "/version",
    tag = "server",
    responses(
        (
            status = OK,
            description = "接続を許可するクライアントのプロトコルバージョンの一覧",
            body = Vec<usize>,
        ),
    ),
)]
pub async fn version() -> Response {
    JsonResponse {
        status_code: StatusCode::OK,
        content: ALLOWED_CLIENT_VERSIONS,
    }
    .into_response()
}

/// 新しい部屋を作成する
#[utoipa::path(
    post,
    path = "/new",
    tag = "rooms",
    request_body = HttpRoomConfig,
    responses(
        (status = CREATED, description = "部屋を作成した", body = CreateRoom),
        (status = BAD_REQUEST, description = "本文または設定が不正", body = CreateRoom),
    ),
)]
pub async fn new_room(body: Result<Json<HttpRoomConfig>, JsonRejection>) -> Response {
    let config = match body {
        Ok(Json(config)) => RoomConfig::try_from(config),
        Err(_) => Err(CreateRoomError::InvalidBody),
    };
    let config = match config {
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

/// 存在する部屋をすべて返す
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
    room_id: Result<Path<RoomId>, PathRejection>,
    req: Request,
    next: Next,
) -> Response {
    let exists = room_id.is_ok_and(|Path(room_id)| GAME_SESSION_MAP.read().contains_key(&room_id));
    if !exists {
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
        JsonResponse {
            status_code: StatusCode::OK,
            content: session.get_room().to_data(room_id),
        }
        .into_response()
    })
}

/// 対局用の WebSocket に接続する
///
/// 接続後にやり取りするメッセージは `PlayerAction` / `Responses` / `RegisterRoomEvent` を参照
#[utoipa::path(
    get,
    path = "/ws",
    tag = "websocket",
    params(("room_id" = RoomId, Path), WsParams),
    responses(
        (status = SWITCHING_PROTOCOLS, description = "WebSocket に切り替えた"),
        (
            status = BAD_REQUEST,
            description = "`v` が指定されていないか未対応 / WebSocket へのアップグレードを要求していない",
        ),
        (
            status = UPGRADE_REQUIRED,
            description = "HTTP/1.0 などで接続を切り替えられない",
        ),
        (status = NOT_FOUND, description = "部屋が存在しない", body = Null),
    ),
)]
pub async fn serve_ws(
    Path(room_id): Path<RoomId>,
    params: Result<Query<WsParams>, QueryRejection>,
    ws: WebSocketUpgrade,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
) -> Response {
    if !params.is_ok_and(|Query(params)| ALLOWED_CLIENT_VERSIONS.contains(&params.v)) {
        return StatusCode::BAD_REQUEST.into_response();
    }
    ws.on_upgrade(move |socket| async move {
        let ip = addr.ip();
        log_ws(ip, WebSocketAction::Connect);
        handle_socket(socket, ip, room_id).await;
        log_ws(ip, WebSocketAction::Disconnect);
    })
}
