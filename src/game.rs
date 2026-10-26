use std::time::Duration;

use crate::game::session::map::GAME_SESSION_MAP;

/// 部屋の作成とWebSocket通信への誘導
pub mod http;

/// 部屋情報の保持とロジックの定義
mod session;

/// game::sessionで定義された部屋情報の操作
mod ws;

const ALLOWED_CLIENT_VERSIONS: &'static [usize] = &[1];

const QUEUE_MESSAGE_LIMIT: usize = 16;
const INITIAL_NUMBER: u8 = 3;

const PLAYER_GRACE_PERIOD: Duration = Duration::from_secs(60);
const PLAYER_EVICTION_INTERVAL: Duration = Duration::from_secs(15);

const PING_INTERVAL: Duration = Duration::from_secs(30);

pub fn spawn_player_eviction() {
    tokio::spawn(async {
        let mut interval = tokio::time::interval(PLAYER_EVICTION_INTERVAL);
        loop {
            interval.tick().await;
            for session in GAME_SESSION_MAP.write().values_mut() {
                session.get_room_mut().evict_expired_players();
            }
        }
    });
}
