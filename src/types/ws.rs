use serde::{
    Deserialize, Serialize,
    de::{self, DeserializeOwned},
    ser::SerializeMap as _,
};
use serde_json::Value;
use utoipa::{
    PartialSchema, ToSchema,
    openapi::{OneOfBuilder, Ref, RefOr, Schema},
};
use uuid::Uuid;

use crate::openapi::{failure, movement, result, tagged, tuple, uuid};

use super::{Position, Side};

// client -> server

pub enum PlayerActionWithoutAuth {
    JoinTeam(Side),
    Resume(Uuid),
}

pub enum PlayerActionWithAuth {
    LeaveTeam,
    Move(Position, Position),
}

pub enum PlayerAction {
    WithoutAuth(PlayerActionWithoutAuth),
    WithAuth(PlayerActionWithAuth),
}

fn deserialize_content<T, E>(content: Option<Value>) -> Result<T, E>
where
    T: DeserializeOwned,
    E: de::Error,
{
    let content = content.ok_or_else(|| E::missing_field("c"))?;
    serde_json::from_value(content).map_err(E::custom)
}

impl<'de> Deserialize<'de> for PlayerAction {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct Tagged {
            t: u8,
            #[serde(default)]
            c: Option<Value>,
        }

        let Tagged { t, c } = Tagged::deserialize(deserializer)?;
        match t {
            0 => Ok(Self::WithoutAuth(PlayerActionWithoutAuth::JoinTeam(
                deserialize_content(c)?,
            ))),
            1 => Ok(Self::WithoutAuth(PlayerActionWithoutAuth::Resume(
                deserialize_content(c)?,
            ))),
            2 => Ok(Self::WithAuth(PlayerActionWithAuth::LeaveTeam)),
            3 => {
                let (old_position, new_position) = deserialize_content(c)?;
                Ok(Self::WithAuth(PlayerActionWithAuth::Move(
                    old_position,
                    new_position,
                )))
            }
            _ => Err(de::Error::invalid_value(
                de::Unexpected::Unsigned(t.into()),
                &"a known player action tag",
            )),
        }
    }
}

impl PartialSchema for PlayerAction {
    fn schema() -> RefOr<Schema> {
        OneOfBuilder::new()
            .description(Some("クライアントからサーバーへ送るメッセージ"))
            .item(tagged(
                0,
                "指定した陣営に参加する",
                Some(Ref::from_schema_name(Side::name()).into()),
            ))
            .item(tagged(
                1,
                "`private_id` を示して切断前のプレイヤーに復帰する",
                Some(uuid().into()),
            ))
            .item(tagged(2, "陣営から抜ける", None))
            .item(tagged(3, "駒を動かす", Some(movement().into())))
            .into()
    }
}

impl ToSchema for PlayerAction {
    fn schemas(schemas: &mut Vec<(String, RefOr<Schema>)>) {
        schemas.push((Side::name().into(), Side::schema()));
    }
}

// server -> client

/// 陣営への参加結果
///
/// `success` で成功と失敗を見分ける
#[derive(Clone, Copy)]
pub enum CreateUser {
    Ok { private_id: Uuid, public_id: Uuid },
    Err { message: &'static str },
}

impl Serialize for CreateUser {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut map = serializer.serialize_map(Some(3))?;
        match self {
            Self::Ok {
                private_id,
                public_id,
            } => {
                map.serialize_entry("success", &true)?;
                map.serialize_entry("private_id", private_id)?;
                map.serialize_entry("public_id", public_id)?;
            }
            Self::Err { message } => {
                map.serialize_entry("success", &false)?;
                map.serialize_entry("message", message)?;
            }
        }
        map.end()
    }
}

impl PartialSchema for CreateUser {
    fn schema() -> RefOr<Schema> {
        OneOfBuilder::new()
            .item(
                result(true)
                    .description(Some("陣営に参加した"))
                    .property(
                        "private_id",
                        uuid().description(Some(
                            "本人だけが知る ID (切断後の復帰時に `Resume` で示す)",
                        )),
                    )
                    .required("private_id")
                    .property(
                        "public_id",
                        uuid().description(Some("他のプレイヤーにも公開される ID")),
                    )
                    .required("public_id"),
            )
            .item(
                failure(["PLAYER_LIMIT_EXCEEDED"])
                    .description(Some("チームが満員で参加できなかった")),
            )
            .into()
    }
}

impl ToSchema for CreateUser {}

#[derive(Clone, Copy)]
pub enum Responses {
    UserCreated(CreateUser),
    Authorized,
    ActionNotAccepted,
    SessionExpired,

    AuthorizedInternal(Uuid),
    GotInvalidData,
}

impl Serialize for Responses {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let (tag, content) = match self {
            Self::UserCreated(user) => (101u8, Some(user)),
            Self::Authorized => (102, None),
            Self::ActionNotAccepted => (103, None),
            Self::SessionExpired => (104, None),
            Self::AuthorizedInternal(_) | Self::GotInvalidData => unreachable!(),
        };
        let mut map = serializer.serialize_map(Some(1 + usize::from(content.is_some())))?;
        map.serialize_entry("t", &tag)?;
        if let Some(content) = content {
            map.serialize_entry("c", content)?;
        }
        map.end()
    }
}

impl PartialSchema for Responses {
    fn schema() -> RefOr<Schema> {
        OneOfBuilder::new()
            .description(Some("サーバーから個別のクライアントへ送るメッセージ"))
            .item(tagged(
                101,
                "`JoinTeam` を処理した",
                Some(Ref::from_schema_name(CreateUser::name()).into()),
            ))
            .item(tagged(102, "`Resume` による復帰を受け入れた", None))
            .item(tagged(103, "直前の操作を受け付けなかった", None))
            .item(tagged(
                104,
                "指定されたプレイヤーが既に部屋から居なくなっている (再度 `JoinTeam` が必要)",
                None,
            ))
            .into()
    }
}

impl ToSchema for Responses {
    fn schemas(schemas: &mut Vec<(String, RefOr<Schema>)>) {
        schemas.push((CreateUser::name().into(), CreateUser::schema()));
        <CreateUser as ToSchema>::schemas(schemas);
    }
}

/// 部屋にいる全員へ配信されるイベント
#[derive(Debug, Clone)]
pub enum RoomEvent {
    MovePiece(bool, (Position, Position)),
    PlayerJoin(Side, String),
    PlayerLeave,
}

impl Serialize for RoomEvent {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut map = serializer.serialize_map(None)?;
        match self {
            Self::MovePiece(toggle_turn, positions) => {
                map.serialize_entry("t", &3u8)?;
                map.serialize_entry("c", &(toggle_turn, positions))?;
            }
            Self::PlayerJoin(side, name) => {
                map.serialize_entry("t", &4u8)?;
                map.serialize_entry("c", &(side, name))?;
            }
            Self::PlayerLeave => map.serialize_entry("t", &5u8)?,
        }
        map.end()
    }
}

impl PartialSchema for RoomEvent {
    fn schema() -> RefOr<Schema> {
        OneOfBuilder::new()
            .description(Some("部屋にいる全員へ配信されるイベント"))
            .item(tagged(
                3,
                "駒が動いた (`c` は `[手番が移るか, [移動元, 移動先]]`)",
                Some(tuple([bool::schema(), movement().into()]).into()),
            ))
            .item(tagged(
                4,
                "プレイヤーが参加した (`c` は `[陣営, 名前]`)",
                Some(tuple([Ref::from_schema_name(Side::name()).into(), String::schema()]).into()),
            ))
            .item(tagged(5, "プレイヤーが抜けた", None))
            .into()
    }
}

impl ToSchema for RoomEvent {
    fn schemas(schemas: &mut Vec<(String, RefOr<Schema>)>) {
        schemas.push((Side::name().into(), Side::schema()));
    }
}

/// 誰による操作なのかを添えた [`RoomEvent`]
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct RegisterRoomEvent {
    /// イベントを起こしたプレイヤーの `public_id`
    #[serde(rename = "i")]
    pub public_id: Uuid,
    #[serde(flatten)]
    pub event: RoomEvent,
}
