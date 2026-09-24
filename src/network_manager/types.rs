use serde::{Deserialize, Serialize};
use tokio::sync::{broadcast};
use crate::player::types::Song;

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum NetworkMessage {
    Auth { code: String },
    AuthOk,
    AuthFail,
    Message{msg: String},
    Seek{position: String},
    CurrentState{song: Option<Song>},
    Play(Song),
    PlayPause,
    Next(Song),
    Previous(Song),
    PlayLater(Song),
    PlayNext(Song),
}

impl NetworkMessage {
    pub fn to_wire(&self) -> String {
        let mut s = serde_json::to_string(self).expect("serialization is infallible");
        s.push('\n');
        s
    }
}


#[derive(Clone)]
pub struct HostBroadcaster {
    pub tx: broadcast::Sender<NetworkMessage>,
}

impl HostBroadcaster {
    pub fn broadcast(&self, msg: NetworkMessage) {
        // `send` only fails when there are zero receivers, which is fine.
        let _ = self.tx.send(msg);
    }
}
