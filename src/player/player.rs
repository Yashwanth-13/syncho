use super::types::*;
use super::cider::*;
use dict::DictIface;
use std::time::{Duration, SystemTime};
pub use crate::player::types::PlayState;
use super::spotify::SpotifyPlayer;

impl PlayState {

    pub fn new(config: &Config) -> PlayState {
        let player = match config {
            Config::Cider { token } => {
                Player::Cider(CiderControl::new(token))
            }

            Config::Spotify { refresh_token, client_id, client_secret, access_token } => {
                Player::Spotify(SpotifyPlayer::new(
                    refresh_token.clone(),
                    client_id.clone(),
                    client_secret.clone(),
                    access_token.clone(),
                ))
            }
        };

        PlayState {
            is_playing: false,
            player,
            current_song: None,
        }
    }

    pub async fn get_current_song(&mut self) -> Option<Song> {
        if !self.is_playing {
            return None;
        }

        match &mut self.player {
            Player::Cider(client) => {
                let result = client.get_current_song().await;
                if let Some(song_attr) = result {
                    let song_name = song_attr.get("song_name").unwrap().to_string();
                    let artist_name = song_attr.get("artist_name").unwrap().to_string();
                    let album_name = song_attr.get("album_name").unwrap().to_string();
                    let current_position_seconds = song_attr.get("position").unwrap().to_string();
                    Some(Song { song_name: song_name, artist_name: artist_name, album_name: album_name, time_started: SystemTime::now().checked_sub(Duration::from_secs(current_position_seconds.parse().unwrap())).unwrap() })
                } else {
                    None
                }
            },

            Player::Spotify(client) => {
                let result = client.get_current_song().await;
                if let Some(song_attr) = result {
                    let song_name = song_attr.get("song_name").unwrap().to_string();
                    let artist_name = song_attr.get("artist_name").unwrap().to_string();
                    let album_name = song_attr.get("album_name").unwrap().to_string();
                    let current_position_seconds = song_attr.get("position").unwrap().to_string();
                    Some(Song { song_name: song_name, artist_name: artist_name, album_name: album_name, time_started: SystemTime::now().checked_sub(Duration::from_secs(current_position_seconds.parse().unwrap())).unwrap() })
                } else {
                    None
                }
            }
        }
    }

    // TODO - REPL
    pub async fn play_pause(&self) {
        match &self.player {
            Player::Cider(client) => {
                client.play_pause().await;
            },
            Player::Spotify(_) => {
                // TODO: Spotify implementation
            }
        }
    }

    // TODO - REPL
    pub async fn play(&self, song: Song) {
        match &self.player {
            Player::Cider(client) => {
                client.play(&song).await;
            },
            Player::Spotify(_) => {
                // TODO: Spotify implementation
            }
        }
    }
}
