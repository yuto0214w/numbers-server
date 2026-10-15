use tokio::sync::broadcast;

use crate::types::{RoomConfig, ws::RegisterRoomEvent};

use self::room::Room;

use super::QUEUE_MESSAGE_LIMIT;

pub mod map;
pub mod room;

#[derive(Debug)]
pub struct GameSession {
    room: Room,
    room_queue: broadcast::Sender<RegisterRoomEvent>,
    _room_queue_recv: broadcast::Receiver<RegisterRoomEvent>,
}

impl GameSession {
    pub fn new(config: RoomConfig) -> Self {
        let (room_queue, _room_queue_recv) = broadcast::channel(QUEUE_MESSAGE_LIMIT);
        Self {
            room: Room::new(config, room_queue.clone()),
            room_queue,
            _room_queue_recv,
        }
    }

    pub fn subscribe_queue(&self) -> broadcast::Receiver<RegisterRoomEvent> {
        self.room_queue.subscribe()
    }

    pub fn get_room(&self) -> &Room {
        &self.room
    }

    pub fn get_room_mut(&mut self) -> &mut Room {
        &mut self.room
    }
}
