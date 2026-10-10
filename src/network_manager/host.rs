use super::types::*;
use futures::StreamExt;
use futures::lock::Mutex;
use tokio::io::{AsyncBufReadExt, BufReader, BufWriter};
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{broadcast};
use std::net::SocketAddr;
use std::sync::{Arc};

use crate::network_manager::helpers::{is_same_player, make_song, process_event, send};
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
                if is_same_player(&target_player, &player_name) {
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
                println!("[syncho] Client connecting from {}", &addr);
                let rx = tx.subscribe();
                tokio::spawn(handle_client(stream, addr, Arc::clone(&play_state), Arc::clone(&session_code), rx));
            }
            Err(e) => {
                eprintln!("[syncho] Accept error: {}", e);
            }
        }
    }
}


async fn handle_client(
    stream: TcpStream,
    client_addr: SocketAddr,
    play_state: Arc<Mutex<PlayState>>,
    session_code: Arc<String>,
    mut rx: broadcast::Receiver<NetworkMessage>,
) {
    let (read_half, write_half) = stream.into_split();
    let reader = Arc::new(Mutex::new(BufReader::new(read_half)));
    let writer: Arc<Mutex<BufWriter<OwnedWriteHalf>>> = Arc::new(Mutex::new(BufWriter::new(write_half)));
    let mut line = String::new();
    
    let res = {
        reader.lock().await.read_line(&mut line).await.unwrap_or(0)
    }; 
    
    if res == 0 {
        return;
    }

    let authed = match serde_json::from_str::<NetworkMessage>(line.trim()) {
        Ok(NetworkMessage::Auth { code }) => {
            code == *session_code
        },
        _ => false,
    };

    if !authed {
        let _ = send(&writer, NetworkMessage::AuthFail).await;
        println!("[syncho] Client failed authentication. - {}", &client_addr);
        return;
    }

    let _ = send(&writer, NetworkMessage::AuthOk).await;
    println!("[syncho] Client authenticated successfully. - {}", &client_addr);

    let curr_song = {
        play_state.lock().await.get_current_song().await
    };

    let message = match curr_song {
        Some(song) => NetworkMessage::CurrentState{song: Some(song)},
        None => NetworkMessage::CurrentState{song: None}
    };

    let _ = send(&writer, message).await;

    // Client message listener
    tokio::spawn(read_client_stream(client_addr, Arc::clone(&reader), Arc::clone(&writer), play_state));

    // Forward broadcast messages
    loop {
        match rx.recv().await {
            Ok(msg) => {
                let res = send(&writer, msg).await;
                match res {
                    Err(e) => {
                        println!("[syncho] Broadcast error - {e}");
                    }

                    _ => {}
                }
            }
            Err(broadcast::error::RecvError::Lagged(n)) => {
                eprintln!("[syncho] Client lagged, dropped {} messages.", n);
            }
            Err(broadcast::error::RecvError::Closed) => break,
        }
    }
}

// Listen to messages from clients
async fn read_client_stream(client_addr: SocketAddr, reader: Arc<Mutex<BufReader<OwnedReadHalf>>>, writer: Arc<Mutex<BufWriter<OwnedWriteHalf>>>, play_state: Arc<Mutex<PlayState>>) {
    let mut line = String::new();
    loop {
        line.clear();
        let read_bytes = {
            reader.lock().await.read_line(&mut line).await
        };

        match read_bytes {
            Ok(0) => {
                println!("[syncho] Client disconnected - {}", &client_addr);
                return;
            }

            _ => {}
        }
        let msg: NetworkMessage = match serde_json::from_str(line.trim()) {
            Ok(m) => m,
            Err(e) => {
                eprintln!("[syncho] Bad message from client: {} — {:?}", e, line);
                continue;
            }
        };

        process_event(msg, Arc::clone(&play_state), Some(Arc::clone(&writer))).await;
    }
}

