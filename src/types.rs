use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

pub mod http;
pub mod ws;

mod room_id;
pub use room_id::RoomId;

pub const BOARD_SIZE: usize = 8;

pub type Position = (usize, usize);
pub type BoardPieces = [[Option<Piece>; BOARD_SIZE]; BOARD_SIZE];

/// 陣営
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum Side {
    #[default]
    A,
    B,
}

impl Side {
    pub fn random() -> Self {
        if rand::random_bool(0.5) {
            Self::A
        } else {
            Self::B
        }
    }

    pub fn reverse(self) -> Self {
        match self {
            Self::A => Self::B,
            Self::B => Self::A,
        }
    }
}

/// 部屋に参加しているプレイヤー
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct Player {
    /// 他のプレイヤーにも公開される ID
    pub public_id: Uuid,
    pub name: String,
    pub side: Side,
}

/// 盤上の駒
#[derive(Debug, Clone, Copy, Serialize, ToSchema)]
pub struct Piece {
    pub side: Side,
    /// 駒が持つ数字
    pub number: u8,
}
