use serde::{Deserialize, Serialize, ser::SerializeMap as _};
use utoipa::{
    PartialSchema, ToSchema,
    openapi::{OneOfBuilder, Ref, RefOr, Schema},
};

use crate::openapi::{failure, result};

use super::{BoardPieces, Piece, Player, RoomId, Side};

// receive

/// 部屋の作成時に指定できる設定 (どちらも省略可能)
#[derive(Deserialize, ToSchema)]
pub struct HttpRoomConfig {
    /// 1 チームあたりの人数の上限 (省略時は 1)
    #[schema(minimum = 1, maximum = 2, example = 1)]
    pub team_player_limit: Option<usize>,
    /// 先手の陣営 (省略時はランダムに決定される)
    pub first_side: Option<Side>,
}

// send

/// 部屋の作成結果
///
/// `success` で成功と失敗を見分ける
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

impl PartialSchema for CreateRoom {
    fn schema() -> RefOr<Schema> {
        OneOfBuilder::new()
            .item(
                result(true)
                    .description(Some("部屋を作成した"))
                    .property("room_id", Ref::from_schema_name(RoomId::name()))
                    .required("room_id"),
            )
            .item(
                failure(["INVALID_PLAYER_LIMIT"]).description(Some("設定が不正で作成できなかった")),
            )
            .into()
    }
}

impl ToSchema for CreateRoom {
    fn schemas(schemas: &mut Vec<(String, RefOr<Schema>)>) {
        schemas.push((RoomId::name().into(), RoomId::schema()));
    }
}

/// 部屋の現在の状態
#[derive(Serialize, ToSchema)]
pub struct RoomData<'a> {
    pub room_id: RoomId,
    /// 現在手番の陣営
    pub current_turn: Side,
    pub players: Vec<&'a Player>,
    /// `pieces[y][x]` で参照する 8x8 の盤面で，駒が無いマスは `null`
    #[schema(value_type = [[Option<Piece>; 8]; 8])]
    pub pieces: &'a BoardPieces,
}

/// 部屋一覧に載る各部屋の概要
#[derive(Serialize, ToSchema)]
pub struct RoomSummary {
    pub id: RoomId,
    /// 参加しているプレイヤーの名前
    pub players: Vec<String>,
}

/// 存在する全ての部屋
#[derive(Serialize, ToSchema)]
pub struct RoomList(pub Vec<RoomSummary>);
