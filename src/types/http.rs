use serde::{Deserialize, Serialize, ser::SerializeMap as _};

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

pub enum CreateRoom {
    Ok { room_id: RoomId },
    Err { message: &'static str },
}

impl Serialize for CreateRoom {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut map = serializer.serialize_map(Some(2))?;
        match self {
            Self::Ok { room_id } => {
                map.serialize_entry("success", &true)?;
                map.serialize_entry("room_id", room_id)?;
            }
            Self::Err { message } => {
                map.serialize_entry("success", &false)?;
                map.serialize_entry("message", message)?;
            }
        }
        map.end()
    }
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
