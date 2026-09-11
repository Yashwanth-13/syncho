use std::time::{SystemTime};
use cider_api::{CiderClient};

pub struct CiderControl {
    pub cider: CiderClient
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