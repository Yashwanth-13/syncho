use cider_api::{CiderClient};
use serde::{self, Deserialize, Serialize};
use super::spotify::SpotifyPlayer;


pub struct CiderControl {
    pub cider: CiderClient
}

pub enum Player {
    Cider(CiderControl),
    Spotify(SpotifyPlayer),
}

pub struct PlayState {
    pub player: Player,
}


#[derive(Serialize, Deserialize, Debug)]
pub enum Config {
    Cider {
        token: String,
    },
    Spotify {
        refresh_token: String,
        client_id: String,
        client_secret: String,
        access_token: String

    },
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Song {
    pub song_name: String,
    pub artist_name: String,
    pub album_name: String,
    // pub time_started: SystemTime
    pub position: u64
}
