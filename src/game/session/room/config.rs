use crate::{
    game::MAX_TEAM_PLAYER_LIMIT,
    types::{
        Side,
        http::{CreateRoomError, HttpRoomConfig},
    },
};

#[derive(Debug, Clone)]
pub struct RoomConfig {
    pub team_player_limit: usize,
    pub first_side: Side,
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
