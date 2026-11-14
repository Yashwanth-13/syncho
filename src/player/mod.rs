pub mod cider;
pub mod player;
pub mod spotify;
pub mod types;

use tokio::sync::{Mutex};
use nowhear::Track;
use std::sync::Arc;
use futures_lite::StreamExt;
use nowhear::{MediaEvent, MediaSource, MediaSourceBuilder, PlaybackState};

use crate::network_manager::Broadcaster;
use crate::network_manager::NetworkMessage;
use crate::player::types::PlayState;
use crate::player::types::Song;

/// String matcher to tell if two player names are the same
pub fn is_same_player(target_player: &String, current_player: &str) -> bool {
    current_player.to_lowercase().contains(target_player)
}

/// Watches player to update its status in `PlayState`
pub async fn player_watcher(play_state: Arc<Mutex<PlayState>>) {
    let source = MediaSourceBuilder::new().build().await.unwrap();
    let mut stream= source.event_stream().await.unwrap();

    let target_player = play_state.lock().await.get_player_str();
    match source.get_player(&target_player).await {
        Ok(_) => {
            play_state.lock().await.update_player_status(true);
        }, // Our music player is running
        
        Err(_) => {
            play_state.lock().await.update_player_status(false);
        } // Our music player is not open
    };    

    while let Some(event) = stream.next().await {
        match event {
            MediaEvent::PlayerAdded { 
                player_name 
            } if is_same_player(&target_player, &player_name) => {
                play_state.lock().await.update_player_status(true); // Set the player state to available
            }
            
            MediaEvent::PlayerRemoved { 
                player_name 
            } if is_same_player(&target_player, &player_name) => {
                play_state.lock().await.update_player_status(false); // Set the player state to not available
            }

            _ => {}
        }
    }
}

/// Listens to playback changes
pub async fn listen_to_playback(broadcaster: Broadcaster, play_state: Arc<Mutex<PlayState>>) {
    let source = MediaSourceBuilder::new().build().await.unwrap();
    let mut stream= source.event_stream().await.unwrap();
    let target_player = play_state.lock().await.get_player_str();

    
    // Do something only when the state change is from our player
    while let Some(event) = stream.next().await {
        match event {
            MediaEvent::TrackChanged { player_name, track } => {
                if is_same_player(&target_player, &player_name) {
                    let song = make_song(track);
                    println!(
                        "Song changed to - {} by {} from {}",
                        song.song_name, song.artist_name, song.album_name
                    );

                    broadcaster.broadcast(NetworkMessage::Play(song.clone()));
                }
            }

            MediaEvent::StateChanged { player_name, state } => {
                if is_same_player(&target_player, &player_name) {
                    match state {
                        PlaybackState::Playing => broadcaster.broadcast(NetworkMessage::Resume),
                        _ => broadcaster.broadcast(NetworkMessage::Pause)
                    }
                }
            }

            MediaEvent::PositionChanged {
                player_name,
                position,
            } if is_same_player(&target_player, &player_name) => {
                broadcaster.broadcast(NetworkMessage::Seek {
                    position: position.as_millis().to_string(),
                });
            }

            _ => {}
        }
    }
}

/// Gives a `Song` from a `nowhear::types::Track`
fn make_song(track: Track) -> Song {
    let album = track.album;
    let album_name = match album {
        Some(name) => name,
        None => "".to_string(),
    };

    Song {
        song_name: track.title.clone(),
        artist_name: track.artist.first().cloned().unwrap_or_default(),
        album_name,
        position: 0,
    }
}