use super::*;
use crate::helpers::format_song;
use crate::player::types::PlayState;
use crate::player::types::Song;
use tokio::sync::{Mutex};
use nowhear::Track;
use std::sync::Arc;

use futures_lite::StreamExt;
use nowhear::{MediaEvent, MediaSource, MediaSourceBuilder, PlaybackState};

pub fn is_same_player(target_player: &String, current_player: &str) -> bool {
    current_player.to_lowercase().contains(target_player)
}


// Listens to playback events
pub async fn listen_to_playback(broadcaster: Broadcaster, play_state: Arc<Mutex<PlayState>>) {
    let source = MediaSourceBuilder::new().build().await.unwrap();
    let mut stream= source.event_stream().await.unwrap();

    let target_player = play_state.lock().await.get_player_str();
    match source.get_player(&target_player).await {
        Ok(player) => {
            play_state.lock().await.update_player_status(true);
        }, // Our music player is running
        
        Err(_) => {
            play_state.lock().await.update_player_status(false);
        } // Our music player is not open
    };

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
                println!("State - {:?}", state);
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


pub fn make_song(track: Track) -> Song {
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

pub async fn send(
    writer: &mut iroh::endpoint::SendStream,
    msg: NetworkMessage,
) -> anyhow::Result<()> {
    let payload = msg.to_bytes();
    let size = payload.len() as u32;
    
    // println!("Writing size: {}", size);

    writer.write_all(&size.to_be_bytes()).await?; // Write the message size first
    writer.write_all(&payload).await?; // Write the actual message

    Ok(())
}

pub async fn read(recv: &mut iroh::endpoint::RecvStream) -> anyhow::Result<NetworkMessage> {
    let mut len = [0u8; 4];
    recv.read_exact(&mut len).await?; // Get the message size first
    
    let len = u32::from_be_bytes(len) as usize;
    // println!("Reading size - {}", len);
    anyhow::ensure!(len <= 1000, "frame too large");
    
    let mut buf = vec![0u8; len];
    recv.read_exact(&mut buf).await?; // Read the actual message
    Ok(NetworkMessage::from_bytes(&buf))
}

/// Process the receieved event and return if the connection needs to be closed (ik its dumb for every NetworkMessage but it is what it is)
pub async fn process_event(
    msg: NetworkMessage,
    play_state: &Arc<Mutex<PlayState>>,
    writer: Option<&mut iroh::endpoint::SendStream>,
)-> bool {
    let mut ps = play_state.lock().await;

    match msg {
        NetworkMessage::Seek { position } => {
            let pos: u64 = position.parse().unwrap();
            ps.seek(pos).await;
            true
        }

        NetworkMessage::CurrentState { song } => {
            match song {
                Some(host_song) => {
                    println!("[syncho] Playing host's current song: {}", format_song(&host_song));
                    ps.play(host_song.clone()).await;
                    ps.seek(host_song.position).await;
                }
                _ => {
                    println!("[syncho] Host is playing nothing right now");
                }
            }
            true
        }

        NetworkMessage::Play(song) => {
            ps.play(song).await;
            true
        }

        NetworkMessage::GetQueue => {
            let queue = ps.get_queue().await;
            let _ = send(&mut writer.unwrap(), NetworkMessage::Queue { queue }).await;
            true
        }

        NetworkMessage::PlayNext(song) => {
            ps.play_next(song).await;
            true
        }

        NetworkMessage::Queue { queue } => {
            for (index, song) in (1..).zip(queue.into_iter()) {
                println!("{index} - {song}");
            }
            true
        }

        NetworkMessage::Bye => {
            writer.unwrap().finish();
            false
        }

        _ => {true}
    }
}
