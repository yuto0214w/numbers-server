use serde::{Deserialize, Serialize};

use super::{BoardPieces, Player, RoomId, Side};

// receive

#[derive(Deserialize)]
pub struct HttpBoardConfig {
    pub team_player_limit: Option<usize>,
    pub first_side: Option<Side>,
}

// send

#[derive(Serialize)]
pub struct ServerInfo {
    pub min_version: usize,
}

#[derive(Serialize)]
#[serde(tag = "success")]
pub enum CreateRoom {
    #[serde(rename = true)]
    Ok { room_id: RoomId },
    #[serde(rename = false)]
    Err { message: &'static str },
}

#[derive(Serialize)]
pub struct RoomData<'a> {
    pub room_id: RoomId,
    pub current_turn: Side,
    pub players: Vec<&'a Player>,
    pub pieces: &'a BoardPieces,
}

#[derive(Serialize)]
pub struct RoomSummary {
    pub id: RoomId,
    pub players: Vec<String>,
}

#[derive(Serialize)]
pub struct RoomList(pub Vec<RoomSummary>);
