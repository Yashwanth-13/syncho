use super::types::*;
use futures::StreamExt;
use futures::lock::Mutex;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{broadcast};
use std::sync::{Arc};

use crate::network_manager::helpers::{is_same_player, make_song};
use crate::player::types::{PlayState};
use nowhear::{MediaEvent, MediaSource, MediaSourceBuilder, PlaybackState};


/// Returns a [`HostBroadcaster`] you can call from the REPL to push events.
pub async fn start_host(
    listener: TcpListener,
    play_state: Arc<Mutex<PlayState>>,
    session_code: Arc<String>,
) -> HostBroadcaster {
    // Channel with room for 64 in-flight messages.
    let (tx, _rx) = broadcast::channel::<NetworkMessage>(64);
    let broadcaster = HostBroadcaster { tx: tx.clone() };

    tokio::spawn(accept_loop(listener, play_state, session_code, tx));
    
    broadcaster
}

// Listens to playback events
pub async fn listen_to_playback(broadcaster: HostBroadcaster, target_player: String) {
    let source = MediaSourceBuilder::new().build().await.unwrap();
    let mut stream = source.event_stream().await.unwrap();
    
    while let Some(event) = stream.next().await {
        match event {
            MediaEvent::TrackChanged { player_name, track} => {
                println!("Track changed - Player: {:?}", &player_name);
                if is_same_player(&target_player, &player_name) {
                    let song = make_song(track);
                    println!("Song changed to: {} {} {}", song.song_name, song.artist_name, &song.album_name);
                    
                    broadcaster.broadcast(NetworkMessage::Play(song.clone()));
                } // Do something only when the state change is from our player
            },

            MediaEvent::StateChanged { player_name, state } => {
                println!("State changed - Player: {:?}", &player_name);
                if is_same_player(&target_player, &player_name) {
                    match state {
                        PlaybackState::Stopped => {broadcaster.broadcast(NetworkMessage::Message{msg: "Host has no media loaded. Standing by..".to_string()})},
                        _ => {
                            broadcaster.broadcast(NetworkMessage::PlayPause);
                            println!("Play/Pause");
                        }
                    }
                } // Do something only when the state change is from our player
            },

            MediaEvent::PositionChanged { player_name, position } => {
                println!("Position changed - Player: {:?}", &player_name);
                if is_same_player(&target_player, &player_name) {
                    println!("Position changed: {:?}", &position);
                    broadcaster.broadcast(NetworkMessage::Seek{position: position.as_millis().to_string()});
                } // Do something only when the state change is from our player
            },

            _ => {}
        }
    }
}

/// Runs forever accepting incoming TCP connections.
async fn accept_loop(
    listener: TcpListener,
    play_state: Arc<Mutex<PlayState>>,
    session_code: Arc<String>,
    tx: broadcast::Sender<NetworkMessage>,
) {
    loop {
        match listener.accept().await {
            Ok((stream, addr)) => {
                println!("[syncho] Client connecting from {}", addr);
                let rx = tx.subscribe();
                tokio::spawn(handle_client(stream, play_state.clone(), Arc::clone(&session_code), rx));
            }
            Err(e) => {
                eprintln!("[syncho] Accept error: {}", e);
            }
        }
    }
}


async fn handle_client(
    stream: TcpStream,
    play_state: Arc<Mutex<PlayState>>,
    session_code: Arc<String>,
    mut rx: broadcast::Receiver<NetworkMessage>,
) {
    let (read_half, mut writer) = stream.into_split();
    let mut reader = BufReader::new(read_half);
    // let mut writer = BufWriter::new(write_half);
    let mut line = String::new();
    let mut ps = play_state.lock().await;
    // ---- Authentication ----
    line.clear();
    if reader.read_line(&mut line).await.unwrap_or(0) == 0 {
        return; // client disconnected before sending anything
    }

    let authed = match serde_json::from_str::<NetworkMessage>(line.trim()) {
        Ok(NetworkMessage::Auth { code }) => code == *session_code,
        _ => false,
    };

    if !authed {
        let _ = writer
            .write_all(NetworkMessage::AuthFail.to_wire().as_bytes())
            .await;
        println!("[syncho] Client failed authentication.");
        return;
    }

    let _ = writer
        .write_all(NetworkMessage::AuthOk.to_wire().as_bytes())
        .await;
    println!("[syncho] Client authenticated successfully.");

    // let curr_state = get_playback_status(ps.get_player_str()).await;
    // println!("Current-state: {:?}", &curr_state.clone().unwrap());
    let curr_song = ps.get_current_song().await;
    let message = match curr_song {
        Some(song) => NetworkMessage::CurrentState{song: Some(song)},
        None => NetworkMessage::CurrentState{song: None}
    };

    let _ = writer
    .write_all(message.to_wire().as_bytes())
    .await;

    // ---- Forward broadcast messages ----
    loop {
        match rx.recv().await {
            Ok(msg) => {
                if writer
                    .write_all(msg.to_wire().as_bytes())
                    .await
                    .is_err()
                {
                    println!("[syncho] Client disconnected.");
                    break;
                }
            }
            Err(broadcast::error::RecvError::Lagged(n)) => {
                eprintln!("[syncho] Client lagged, dropped {} messages.", n);
            }
            Err(broadcast::error::RecvError::Closed) => break,
        }
    }
}

