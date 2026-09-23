use super::types::*;
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
            player,
        }
    }

    pub fn get_player_str(&self) -> String {
        match self.player {
            Player::Cider(_) => "cider".to_string(),
            Player::Spotify(_) => "spotify".to_string()
        }
    }

    pub async fn get_current_song(&mut self) -> Option<Song> {
        match &mut self.player {
            Player::Cider(client) => {
                client.get_current_song().await
            },

            Player::Spotify(client) => {
                let result = client.get_current_song().await;
                if let Some(song_attr) = result {
                    let song_name = song_attr.get("song_name").unwrap().to_string();
                    let artist_name = song_attr.get("artist_name").unwrap().to_string();
                    let album_name = song_attr.get("album_name").unwrap().to_string();
                    let current_position_seconds = song_attr.get("position").unwrap().to_string();
                    let pos_secs = current_position_seconds.parse::<u64>().unwrap_or(0);
                    Some(Song {
                        song_name,
                        artist_name,
                        album_name,
                        position: pos_secs
                    })
                } else {
                    None
                }
            }
        }
    }

    pub async fn seek(&mut self, new_position: u128) {
        match &mut self.player {
            Player::Cider(client) => {
                client.seek(new_position.try_into().unwrap());
            }

            _ => {}

            // TODO - Seek for spotify
            // Player::Spotify()
        }
    }

    
    pub async fn play(&mut self, song: Song) {
        match &mut self.player {
            Player::Cider(client) => {
                if let Err(err) = client.play(&song).await {
                    eprintln!("Cider Play failed: {}", err )
                }
            },
            
            Player::Spotify(client) => {
                if let Err(e) = client.play(&song).await {
                    eprintln!("Spotify implementation failed: {}", e);
                }
            }
        }
    }


    //addes the song at the end of the queue(laaaaaaaaaaaast)
    pub async fn play_later(&mut self, song: Song) {
        match &mut self.player {
            Player::Cider(client) => {
                client.play_later(&song).await;
            },

            Player::Spotify(client) => {
                if let Err(e) = client.add_to_queue(&song).await {
                    eprintln!("Spotify implementation failed: {}", e);
                }
            }
        }
    }

    pub async fn play_pause(&mut self) {
        match &mut self.player {
            Player::Cider(client) => {
                client.play_pause().await;
            },

            Player::Spotify(client) => {
                if let Err(e) = client.play_pause().await {
                    eprintln!("Spotify implementation failed: {}", e);
                }
            }
        }
    }

    //plays previous song
    pub async fn previous(&mut self) {
        match &mut self.player {
            Player::Cider(client) => {
                client.previous().await;
            },

            Player::Spotify(client) => {
                if let Err(e) = client.previous().await {
                    eprintln!("Spotify implementation failed: {}", e);
                }
            }
        }
    }
    
    //plays the next song
    pub async fn next(&mut self) {
        match &mut self.player {
            Player::Cider(client) => {
                client.next().await;
            },

            Player::Spotify(client) => {
                if let Err(e) = client.next().await {
                    eprintln!("Spotify implementation failed: {}", e);
                }
            }
        }
    }
    
    //addes the song to queue(this is the song that plays immediately after the present one)
    pub async fn play_next(&mut self, song: Song) {
        match &mut self.player {
            Player::Cider(client) => {
                client.play_next(&song).await;
            },

            Player::Spotify(client) => {
                if let Err(e) = client.add_to_queue(&song).await {
                    eprintln!("Spotify implementation failed: {}", e);
                }
            }
        }
    }
}
