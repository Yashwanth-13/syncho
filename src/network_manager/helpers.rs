use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, BufWriter};
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
use super::types::*;
use nowhear::{Track};
use crate::player::types::Song;
use crate::player::player::PlayState;
use std::sync::{Arc};
use futures::lock::{self, Mutex};

pub fn is_same_player(target_player: &String, current_player: &String) -> bool {
    current_player.to_lowercase().contains(target_player)
}

pub fn make_song(track: Track) -> Song {
    let album = track.album;
    let album_name = match album {
        Some(name) => {name}
        None => {"".to_string()}
    };

    Song { song_name: track.title.clone(), artist_name: track.artist.first().cloned().unwrap_or_default(), album_name: album_name, position: 0 }
}

pub async fn send(writer: &Arc<Mutex<BufWriter<OwnedWriteHalf>>>, msg: NetworkMessage) -> std::io::Result<()> {
    let mut locked_wr = writer.lock().await;

    locked_wr.write_all(msg.to_wire().as_bytes()).await?;
    locked_wr.flush().await
}

/// Process the receieved event
pub async fn process_event(msg: NetworkMessage, play_state: Arc<Mutex<PlayState>>, writer: Option<Arc<Mutex<BufWriter<OwnedWriteHalf>>>>) {
    let mut ps = play_state.lock().await;

    match msg {
        NetworkMessage::Message{msg} => {
            println!("[syncho] {}", msg)
        }

        NetworkMessage::Seek{position} => {
            let pos: u64 = position.parse().unwrap();
            ps.seek(pos).await;
        }

        NetworkMessage::CurrentState{song} => {
            match song {
                Some(host_song) => {
                    ps.play(host_song.clone()).await;
                    ps.seek(host_song.position.try_into().unwrap()).await;
                    println!("[syncho] playing Host's current song: {} - {}", host_song.song_name, host_song.artist_name);
                }
                _ => {
                    println!("[syncho] Host is playing nothing right now");
                }
            }
        }

        NetworkMessage::Play(song) => {
            println!("[syncho] Playing: {} — {}", song.song_name, song.artist_name);
            ps.play(song).await;
        }

        NetworkMessage::PlayPause => {
            println!("[syncho] Play/Pause");
            ps.play_pause().await;
        }

        NetworkMessage::GetQueue => {
            let queue = ps.get_queue().await;
            let _ = send(&writer.unwrap(), NetworkMessage::Queue { queue }).await;
        }

        NetworkMessage::PlayNext(song)=> {
            println!("[syncho] + Play next: {}", song.song_name);
            ps.play_next(song).await;
        }

        NetworkMessage::Queue { queue } => {
            let mut index = 1;
            for song in queue {
                println!("{index} - {song}");
                index += 1;
            }
        }

        // Auth messages shouldn't arrive here
        NetworkMessage::Auth { .. } | NetworkMessage::AuthOk | NetworkMessage::AuthFail => {}
    }
}
