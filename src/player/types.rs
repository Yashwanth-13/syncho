use std::time::{SystemTime};
use cider_api::{CiderClient};
use serde::{self, Deserialize, Serialize};

pub struct CiderControl {
    pub cider: CiderClient
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Config {
    pub player: String,
    pub token: String,
}

pub enum Player {
    Cider(CiderControl),
    Spotify
}

pub struct Song {
    pub song_name: String,
    pub artist_name: String,
    pub album_name: String,
    pub time_started: SystemTime
}