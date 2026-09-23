use super::types::*;
use futures::StreamExt;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{broadcast};
use std::sync::{Arc};

use crate::network_manager::helpers::{get_playback_status, is_same_player};
use crate::player::types::Song;
use nowhear::{MediaEvent, MediaSource, MediaSourceBuilder, PlaybackState};


/// Returns a [`HostBroadcaster`] you can call from the REPL to push events.
pub async fn start_host(
    listener: TcpListener,
    session_code: Arc<String>,
) -> HostBroadcaster {
    // Channel with room for 64 in-flight messages.
    let (tx, _rx) = broadcast::channel::<NetworkMessage>(64);
    let broadcaster = HostBroadcaster { tx: tx.clone() };

    tokio::spawn(accept_loop(listener, session_code, tx));
    
    broadcaster
}

// Listens to playback events
pub async fn listen_to_playback(broadcaster: HostBroadcaster, target_player: String) {
    let source = MediaSourceBuilder::new().build().await.unwrap();
    let mut stream = source.event_stream().await.unwrap();
    
    while let Some(event) = stream.next().await {
        match event {
            MediaEvent::TrackChanged { player_name, track} => {
                if !is_same_player(&target_player, &player_name) {} // Ignore if the state change was from some other player

                let album = track.album;
                let album_name = match album {
                    Some(name) => {name}
                    None => {"".to_string()}
                };
                println!("Song changed to: {} {} {}", track.title, track.artist.concat().clone(), &album_name);
                let song = Song { song_name: track.title.clone(), artist_name: track.artist.concat().clone(), album_name: album_name, position: 0 };
                broadcaster.broadcast(NetworkMessage::Play(song.clone()));
            },

            MediaEvent::StateChanged { player_name, state } => {
                if !is_same_player(&target_player, &player_name) {} // Ignore if the state change was from some other player

                match state {
                    PlaybackState::Stopped => {broadcaster.broadcast(NetworkMessage::Message{msg: "Host has no media loaded. Standing by..".to_string()})},
                    _ => {
                        broadcaster.broadcast(NetworkMessage::PlayPause);
                        println!("Play/Pause");
                    }
                }
            },

            MediaEvent::PositionChanged { player_name, position } => {
                if !is_same_player(&target_player, &player_name) {} // Ignore if the state change was from some other player

                println!("Position changed: {:?}", &position);
                broadcaster.broadcast(NetworkMessage::Seek{position: position.as_millis().to_string()});

            },

            _ => {}
        }
    }
}

/// Runs forever accepting incoming TCP connections.
async fn accept_loop(
    listener: TcpListener,
    session_code: Arc<String>,
    tx: broadcast::Sender<NetworkMessage>,
) {
    loop {
        match listener.accept().await {
            Ok((stream, addr)) => {
                println!("[syncho] Client connecting from {}", addr);
                let code = Arc::clone(&session_code);
                let rx = tx.subscribe();
                tokio::spawn(handle_client(stream, code, rx));
            }
            Err(e) => {
                eprintln!("[syncho] Accept error: {}", e);
            }
        }
    }
}


async fn handle_client(
    stream: TcpStream,
    session_code: Arc<String>,
    mut rx: broadcast::Receiver<NetworkMessage>,
) {
    let (read_half, mut writer) = stream.into_split();
    let mut reader = BufReader::new(read_half);
    // let mut writer = BufWriter::new(write_half);
    let mut line = String::new();

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

    let curr_state = get_playback_status().await;
    println!("{:?}", curr_state.unwrap());
    let _ = writer
        .write_all(NetworkMessage::CurrentState{song: get_playback_status().await}.to_wire().as_bytes())
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

