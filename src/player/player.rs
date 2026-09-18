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
            is_playing: false,
            player,
            current_song: None,
            queue_size: 0
        }
    }
    
    async fn set_current_song(playstate: &mut PlayState) {
        match &mut playstate.player {
            Player::Cider(client) => {
                let result = client.get_current_song().await;
                if let Some(current_song)  = result {
                    playstate.current_song = Some(current_song);
                    playstate.is_playing = true;
                } else {
                    playstate.current_song = None;
                    playstate.is_playing = false;
                }
            },

            Player::Spotify(client) => {
                let result = client.get_current_song().await;
                if let Some(song_attr) = result {
                    let song_name = song_attr.get("song_name").unwrap().to_string();
                    let artist_name = song_attr.get("artist_name").unwrap().to_string();
                    let album_name = song_attr.get("album_name").unwrap().to_string();
                    let current_position_seconds = song_attr.get("position").unwrap().to_string();
                    let pos_secs = current_position_seconds.parse::<u64>().unwrap_or(0);
                    playstate.is_playing = true;
                    playstate.current_song = Some(Song {
                        song_name,
                        artist_name,
                        album_name,
                        position: pos_secs
                    });
                } else {
                    playstate.current_song = None;
                    playstate.is_playing = false;
                }
            }
        }
    }


    pub async fn get_current_song(&self) -> Option<Song> {
        self.current_song.clone()
    }

    
    pub async fn play(&mut self, song: Song) {
        match &mut self.player {
            Player::Cider(client) => {
                if let Ok(()) = client.play(&song).await {
                    PlayState::set_current_song(self);
                }
            },
            Player::Spotify(client) => {
                if let Err(e) = client.play(&song).await {
                    eprintln!("Spotify implementation failed: {}", e);
                } else {
                    PlayState::set_current_song(self);
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

            // TODO - Spotify
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
    
    //addes the song to queue(this is the song that plays immediatly after the present one)
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
