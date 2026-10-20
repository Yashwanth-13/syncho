use super::spotify::SpotifyPlayer;
use super::types::*;
use crate::player::types::PlayState;
use crate::helpers::format_song;

impl PlayState {
    
    // PlayState constructor
    pub async fn new(config: &Config) -> Self {
        let player = match config {
            Config::Cider {token, ..} => Player::Cider(CiderControl::new(token)),

            Config::Spotify {
                username,
                refresh_token,
                client_id,
                client_secret,
                access_token,
            } => Player::Spotify(SpotifyPlayer::new(
                username.clone(),
                refresh_token.clone(),
                client_id.clone(),
                client_secret.clone(),
                access_token.clone(),
            )),
        };
        

        PlayState { player, player_online: false }
    }

    pub fn get_player_str(&self) -> String {
        match self.player {
            Player::Cider(_) => "cider".to_string(),
            Player::Spotify(_) => "spotify".to_string(),
        }
    }

    pub fn update_player_status(&mut self, status: bool) {
        self.player_online = status;
    }

    async fn can_call_player(&self) -> bool {
        self.player_online
    }

    pub async fn get_current_song(&mut self) -> Option<Song> {
        if !self.can_call_player().await {
            println!("[syncho] Your player is not running.");
            return None;
        };

        match &mut self.player {
            Player::Cider(client) => client.get_current_song().await,

            Player::Spotify(client) => {
                let result = client.get_current_song().await;
                if let Some(song_attr) = result {
                    let song_name = song_attr.get("song_name").unwrap().to_string();
                    let artist_name = song_attr.get("artist_name").unwrap().to_string();
                    let album_name = song_attr.get("album_name").unwrap().to_string();
                    let current_position_millis = song_attr.get("position").unwrap().to_string();
                    let pos_millis = current_position_millis.parse::<u64>().unwrap_or(0);
                    Some(Song {
                        song_name,
                        artist_name,
                        album_name,
                        position: pos_millis,
                    })
                } else {
                    None
                }
            }
        }
    }

    // Song seek
    pub async fn seek(&mut self, new_position: u64) {
        if !self.can_call_player().await {
            println!("[syncho] Your player is not running. Cannot seek");
            return;
        };

        match &mut self.player {
            Player::Cider(client) => {
                client.seek(new_position.try_into().unwrap()).await;
            }

            Player::Spotify(client) => {
                if let Err(e) = client.seek(new_position).await {
                    eprintln!("Spotify seek failed: {}", e);
                }
            }
        }
    }

    // Plays the given song
    pub async fn play(&mut self, song: Song) {
        let formatted_song = format_song(&song);
        if !self.can_call_player().await {
            println!("[syncho] Your player is not running. Cannot play - {formatted_song}");
            return;
        };

        println!("[syncho] Playing - {formatted_song}");
        match &mut self.player {
            Player::Cider(client) => {
                if let Err(err) = client.play(&song).await {
                    eprintln!("Cider Play failed: {}", err)
                }
            }

            Player::Spotify(client) => {
                if let Err(e) = client.play(&song).await {
                    eprintln!("Spotify implementation failed: {}", e);
                }
            }
        }
    }

    //Adds the song at the end of the queue(laaaaaaaaaaaast)
    pub async fn play_later(&mut self, song: Song) {
        if !self.can_call_player().await {
            println!(
                "[syncho] Your player is not running. Cannot add - {} by {} from {}",
                &song.song_name, &song.artist_name, &song.album_name
            );
            return;
        };

        match &mut self.player {
            Player::Cider(client) => {
                let _ = client.play_later(&song).await;
            }

            // TODO
            Player::Spotify(client) => {
                if let Err(e) = client.add_to_queue(&song).await {
                    eprintln!("Spotify implementation failed: {}", e);
                }
            }
        }
    }

    pub async fn play_pause(&mut self) {
        if !self.can_call_player().await {
            println!("[syncho] Play/Pause - Your player is not running.");
            return;
        };

        println!("[syncho] Play/Pause");
        match &mut self.player {
            Player::Cider(client) => {
                let _ = client.play_pause().await;
            }

            Player::Spotify(client) => {
                if let Err(e) = client.play_pause().await {
                    eprintln!("Spotify implementation failed: {}", e);
                }
            }
        }
    }

    //Adds a song to play right after the current one
    pub async fn play_next(&mut self, song: Song) {
        let formatted_song = format_song(&song);
        if !self.can_call_player().await {
            println!("[syncho] Your player is not running. Cannot add - {formatted_song}");
            return;
        };

        // TODO - Print the name of the song that the player selected instead of what the user typed
        println!("Playing next - {formatted_song}");
        match &mut self.player {
            Player::Cider(client) => {
                let _ = client.play_next(&song).await;
            }

            Player::Spotify(client) => {
                if let Err(e) = client.add_to_queue(&song).await {
                    eprintln!("Spotify implementation failed: {}", e);
                }
            }
        }
    }

    // Gets the player's queue
    pub async fn get_queue(&mut self) -> Vec<String> {
        if !self.can_call_player().await {
            println!("[syncho] Your player is not running. Cannot fetch queue");
            return Vec::new();
        }

        match &mut self.player {
            Player::Cider(client) => client.get_queue().await,

            Player::Spotify(client) => client.get_queue().await,
        }
    }
}
