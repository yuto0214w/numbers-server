use serde::{Deserialize, Serialize, ser::SerializeMap as _};
use utoipa::{
    IntoParams, PartialSchema, ToSchema,
    openapi::{OneOfBuilder, Ref, RefOr, Schema},
};

use crate::openapi::{board, failure, result};

use super::{BoardPieces, MAX_TEAM_PLAYER_LIMIT, Player, RoomConfig, RoomId, Side};

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

impl TryFrom<HttpRoomConfig> for RoomConfig {
    type Error = CreateRoomError;

    fn try_from(value: HttpRoomConfig) -> Result<Self, Self::Error> {
        let team_player_limit = match value.team_player_limit {
            Some(limit) if limit >= 1 && limit <= MAX_TEAM_PLAYER_LIMIT => limit,
            None => 1,
            _ => return Err(CreateRoomError::InvalidPlayerLimit),
        };
        Ok(Self {
            team_player_limit,
            first_side: value.first_side.unwrap_or_else(Side::random),
        })
    }
}

#[derive(Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct WsParams {
    /// クライアントが対応するプロトコルバージョン
    #[param(minimum = 0)]
    pub v: usize,
}

// send

/// 部屋を作成できなかった理由
#[derive(Debug, Clone, Copy, Serialize, ToSchema)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CreateRoomError {
    InvalidBody,
    InvalidPlayerLimit,
}

/// 部屋の作成結果
///
/// `success` で成功と失敗を見分ける
pub enum CreateRoom {
    Ok { room_id: RoomId },
    Err { message: CreateRoomError },
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
                failure::<CreateRoomError>()
                    .description(Some("本文または設定が不正で作成できなかった")),
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
    pub options: &'a RoomConfig,
    /// 現在手番の陣営
    pub current_turn: Side,
    pub players: Vec<&'a Player>,
    #[schema(schema_with = board)]
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
