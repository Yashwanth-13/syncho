use cider_api::{CiderClient};
use serde::{self, Deserialize, Serialize};
use super::spotify::SpotifyPlayer;


pub struct CiderControl {
    pub cider: CiderClient
}

pub struct PlayState {
    pub is_playing: bool,
    pub player: Player,
    pub current_song: Option<Song>,
    pub queue_size: u64
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

pub enum Player {
    Cider(CiderControl),
    Spotify(SpotifyPlayer),
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Song {
    pub song_name: String,
    pub artist_name: String,
    pub album_name: String,
    // pub time_started: SystemTime
    pub position: u64
}
