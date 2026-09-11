use super::types::*;
use super::cider::*;
use dict::DictIface;
use std::time::{Duration, SystemTime};

pub struct PlayState {
    is_playing: bool,
    player: Player,
    current_song: Song
}

impl PlayState {
    async fn get_current_song(&self) -> Option<Song> {
        if !self.is_playing {
            return None;
        }

        match &self.player {
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

            // TODO 
            _ => {
                println!("spotify implementation");
                Some(Song { song_name: "()".to_string(), artist_name: "()".to_string(), album_name: "()".to_string(), time_started: SystemTime::now() })
            }
        }
    }

    // TODO - REPL
    async fn play_pause(&self) {
        match &self.player {
            Player::Cider(client) => {
                client.play_pause().await;
            },

            // TODO
            _ => {}
        }
    }

    // TODO - REPL
    async fn play(&self, song: Song) {
        match &self.player {
            Player::Cider(client) => {
                client.play(&song).await;
            },
            
            // 
            _ => {}
        }
    }
}