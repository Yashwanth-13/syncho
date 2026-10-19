use super::types::*;
use crate::helpers::format_song;
use crate::player::types::PlayState;
use crate::player::types::Song;
use tokio::sync::{Mutex};
use nowhear::Track;
use std::sync::Arc;
use tokio::io::{AsyncWriteExt};

use futures_lite::StreamExt;
use nowhear::{MediaEvent, MediaSource, MediaSourceBuilder, PlaybackState};

pub fn is_same_player(target_player: &String, current_player: &str) -> bool {
    current_player.to_lowercase().contains(target_player)
}


// Listens to playback events
pub async fn listen_to_playback(broadcaster: HostBroadcaster, play_state: Arc<Mutex<PlayState>>) {
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
                // println!("Track changed - Player: {:?}", &player_name);
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
                // println!("State changed - Player: {:?}", &player_name);
                if is_same_player(&target_player, &player_name) {
                    match state {
                        PlaybackState::Stopped => broadcaster.broadcast(NetworkMessage::Message {
                            msg: "Host has no media loaded. Standing by..".to_string(),
                        }),
                        _ => {
                            broadcaster.broadcast(NetworkMessage::PlayPause);
                            println!("Play/Pause");
                        }
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
    writer: &Arc<Mutex<iroh::endpoint::SendStream>>,
    msg: NetworkMessage,
) -> std::io::Result<()> {
    let mut locked_wr = writer.lock().await;

    locked_wr.write_all(&msg.to_bytes()).await?;
    locked_wr.flush().await
}

/// Process the receieved event
pub async fn process_event(
    msg: NetworkMessage,
    play_state: &Arc<Mutex<PlayState>>,
    writer: Option<&Arc<Mutex<iroh::endpoint::SendStream>>>,
) {
    let mut ps = play_state.lock().await;

    match msg {

        NetworkMessage::Message { msg } => {
            println!("[syncho] {}", msg)
        }

        NetworkMessage::Seek { position } => {
            let pos: u64 = position.parse().unwrap();
            ps.seek(pos).await;
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
        }

        NetworkMessage::Play(song) => {
            ps.play(song).await;
        }

        NetworkMessage::PlayPause => {
            ps.play_pause().await;
        }

        NetworkMessage::GetQueue => {
            let queue = ps.get_queue().await;
            let _ = send(&writer.unwrap(), NetworkMessage::Queue { queue }).await;
        }

        NetworkMessage::PlayNext(song) => {
            ps.play_next(song).await;
        }

        NetworkMessage::Queue { queue } => {
            for (index, song) in (1..).zip(queue.into_iter()) {
                println!("{index} - {song}");
            }
        }

        NetworkMessage::Bye => {

        }

        // Auth messages shouldn't arrive here
        NetworkMessage::Hello | NetworkMessage::Welcome | NetworkMessage::Auth { .. } | NetworkMessage::AuthOk | NetworkMessage::AuthFail => {}
    }
}
