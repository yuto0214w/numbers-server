use std::{collections::HashMap, time::Instant};

use tokio::sync::broadcast;
use uuid::Uuid;

use crate::{
    game::PLAYER_GRACE_PERIOD,
    types::{
        Player, Position, RoomConfig, RoomId, Side,
        http::RoomData,
        ws::{RegisterRoomEvent, RoomEvent},
    },
};

use self::board::{Board, BoardOperationResult};

pub mod board;

#[derive(Debug, Clone)]
pub struct Room {
    config: RoomConfig,
    room_queue: broadcast::Sender<RegisterRoomEvent>,
    players: HashMap<Uuid, Player>,
    disconnected_at: HashMap<Uuid, Instant>,
    board: Board,
}

impl Room {
    pub fn new(config: RoomConfig, room_queue: broadcast::Sender<RegisterRoomEvent>) -> Self {
        Self {
            board: Board::new(config.first_side),
            config,
            room_queue,
            players: HashMap::new(),
            disconnected_at: HashMap::new(),
        }
    }

    fn send_to_queue(&self, public_id: Uuid, event: RoomEvent) {
        self.room_queue
            .send(RegisterRoomEvent { public_id, event })
            .unwrap();
    }

    pub fn create_player(&mut self, side: Side, name: String) -> Option<(Uuid, Uuid)> {
        if self.players.values().filter(|p| p.side == side).count() == self.config.team_player_limit
        {
            return None;
        }
        let private_id = Uuid::new_v4();
        let public_id = Uuid::new_v4();
        self.players.insert(
            private_id,
            Player {
                public_id,
                name: name.to_owned(),
                side,
            },
        );
        self.send_to_queue(public_id, RoomEvent::PlayerJoin(side, name));
        Some((private_id, public_id))
    }

    pub fn remove_player(&mut self, private_id: Uuid) -> Option<Uuid> {
        self.disconnected_at.remove(&private_id);
        let result = self
            .players
            .remove(&private_id)
            .map(|previous_data| previous_data.public_id);
        if let Some(public_id) = result {
            self.send_to_queue(public_id, RoomEvent::PlayerLeave);
        }
        result
    }

    pub fn mark_disconnected(&mut self, private_id: Uuid) {
        if self.players.contains_key(&private_id) {
            self.disconnected_at.insert(private_id, Instant::now());
        }
    }

    pub fn mark_connected(&mut self, private_id: Uuid) {
        self.disconnected_at.remove(&private_id);
    }

    pub fn evict_expired_players(&mut self) {
        let expired: Vec<Uuid> = self
            .disconnected_at
            .iter()
            .filter(|(_, disconnected_at)| disconnected_at.elapsed() >= PLAYER_GRACE_PERIOD)
            .map(|(private_id, _)| *private_id)
            .collect();
        for private_id in expired {
            self.remove_player(private_id);
        }
    }

    pub fn contains_player(&self, private_id: Uuid) -> bool {
        self.players.contains_key(&private_id)
    }

    pub fn get_players(&self) -> Vec<&Player> {
        self.players.values().collect()
    }

    pub fn to_data(&self, room_id: RoomId) -> RoomData<'_> {
        RoomData {
            room_id,
            options: &self.config,
            current_turn: self.board.get_current_turn(),
            players: self.get_players(),
            pieces: self.board.get_pieces(),
        }
    }

    pub fn move_piece(
        &mut self,
        private_id: Uuid,
        old_position: Position,
        new_position: Position,
    ) -> BoardOperationResult<()> {
        let player = self.players.get(&private_id).unwrap();
        let toggle_turn = self
            .board
            .move_piece(player.side, old_position, new_position)?;
        self.send_to_queue(
            player.public_id,
            RoomEvent::MovePiece(toggle_turn, (old_position, new_position)),
        );
        Ok(())
    }
}
