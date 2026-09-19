use clap::builder::NonEmptyStringValueParser;
use futures::StreamExt;
use nowhear::source::PlatformMediaSource;
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, BufWriter};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{broadcast};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::player::player::PlayState;
use crate::player::types::Song;
use nowhear::{MediaEvent, MediaSource, MediaSourceBuilder, PlaybackState};

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum NetworkMessage {
    Auth { code: String },
    AuthOk,
    AuthFail,
    Message{msg: String},
    Seek{position: String},
    CurrentState{song: Option<Song>},
    Play(Song),
    PlayPause,
    Next(Song),
    Previous(Song),
    PlayLater(Song),
    PlayNext(Song),
}

impl NetworkMessage {
    pub fn to_wire(&self) -> String {
        let mut s = serde_json::to_string(self).expect("serialization is infallible");
        s.push('\n');
        s
    }
}


#[derive(Clone)]
pub struct HostBroadcaster {
    tx: broadcast::Sender<NetworkMessage>,
}

impl HostBroadcaster {
    pub fn broadcast(&self, msg: NetworkMessage) {
        // `send` only fails when there are zero receivers, which is fine.
        let _ = self.tx.send(msg);
    }
}


/// Returns a [`HostBroadcaster`] you can call from the REPL to push events.
pub async fn start_host(
    listener: TcpListener,
    play_state: Arc<PlayState>,
    session_code: Arc<String>,
) -> HostBroadcaster {
    // Channel with room for 64 in-flight messages.
    let (tx, _rx) = broadcast::channel::<NetworkMessage>(64);
    let broadcaster = HostBroadcaster { tx: tx.clone() };

    tokio::spawn(accept_loop(listener, session_code, tx));
    
    broadcaster
}

// Listens to playback events
pub async fn listen_to_playback(broadcaster: HostBroadcaster) {
    let source = MediaSourceBuilder::new().build().await.unwrap();
    let mut stream = source.event_stream().await.unwrap();
    
    while let Some(event) = stream.next().await {
        match event {
            MediaEvent::TrackChanged { player_name, track} => {
                let song = Song { song_name: track.title.clone(), artist_name: track.artist.concat(), album_name: track.album.unwrap(), position: 0 };
                broadcaster.broadcast(NetworkMessage::Play(song.clone()));
                println!("Song changed to: {}", track.title);
            },

            MediaEvent::StateChanged { player_name, state } => {
                match state {
                    PlaybackState::Stopped => {broadcaster.broadcast(NetworkMessage::Message{msg: "Host has no media loaded. Standing by..".to_string()})},
                    _ => {
                        broadcaster.broadcast(NetworkMessage::PlayPause);
                        println!("Play/Pause");
                    }
                }
            },

            MediaEvent::PositionChanged { player_name, position } => {
                println!("Position changed: {:?}", &position);
                broadcaster.broadcast(NetworkMessage::Seek{position: position.as_millis().to_string()}); //Assuming the position is in seconds

            },

            _ => {}
        }
    }
}

// Gets current song without involving the players directly - Assumes only Cider or Spotify is playing. 
pub async fn get_playback_status() -> Option<Song> {
    let src = MediaSourceBuilder::new().build().await.unwrap();
    let players = src.list_players().await.unwrap();

    if let Some(player_name) = players.first() {
        let player_info = src.get_player(player_name).await.unwrap();
        if let Some(track) = player_info.current_track {
            return Some(Song { song_name: track.title.clone(), artist_name: track.artist.concat(), album_name: track.album.unwrap(), position: 0 });
        }
    }

    None
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



// Client side
pub async fn join_session(
    host_addr: &str,
    session_code: &str,
    play_state: Arc<Mutex<PlayState>>,
) -> Result<(), Box<dyn std::error::Error>> {
    let stream = TcpStream::connect(host_addr).await?;
    println!("[syncho] Connected to host at {}", host_addr);

    let (read_half, mut write_half) = stream.into_split();
    let mut reader = BufReader::new(read_half);

    // ---- Send auth ----
    let auth_msg = NetworkMessage::Auth {
        code: session_code.to_string(),
    };
    write_half
        .write_all(auth_msg.to_wire().as_bytes())
        .await?;

    // ---- Read auth response ----
    let mut line = String::new();
    reader.read_line(&mut line).await?;
    match serde_json::from_str::<NetworkMessage>(line.trim())? {
        NetworkMessage::AuthOk => {
            println!("[syncho] Authenticated! Receiving sync events…");
        }
        NetworkMessage::AuthFail => {
            eprintln!("[syncho] Wrong session code.");
            return Err("Authentication failed".into());
        }
        other => {
            eprintln!("[syncho] Unexpected first message: {:?}", other);
            return Err("Unexpected auth response".into());
        }
    }

    // ---- Event loop ----
    loop {
        line.clear();
        let n = reader.read_line(&mut line).await?;
        if n == 0 {
            println!("[syncho] Host closed the connection.");
            break;
        }

        let msg: NetworkMessage = match serde_json::from_str(line.trim()) {
            Ok(m) => m,
            Err(e) => {
                eprintln!("[syncho] Bad message from host: {} — {:?}", e, line.trim());
                continue;
            }
        };

        apply_event(msg, Arc::clone(&play_state)).await;
    }

    Ok(())
}

/// Apply a received host event to the local player.
async fn apply_event(msg: NetworkMessage, play_state: Arc<Mutex<PlayState>>) {
    let mut ps = play_state.lock().unwrap();

    match msg {
        NetworkMessage::Message{msg} => {
            println!("[syncho] {}", msg)
        }

        NetworkMessage::Seek{position} => {
            let pos: u128 = position.parse().unwrap();
            ps.seek(pos).await;
        }

        NetworkMessage::CurrentState{song} => {
            match song {
                Some(host_song) => {
                    if let Some(curr_song) = get_playback_status().await {
                        if &curr_song.song_name != &host_song.song_name {
                            ps.play(host_song.clone()).await;
                            println!("Received Song of current state: {:?}", host_song.clone());
                            ps.seek(host_song.position.try_into().unwrap()).await;
                        }
                    }
                }

                _ => {}
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

        NetworkMessage::Next(song)=> {
            println!("[syncho] Skipping to next song {}", song.song_name);
            ps.play(song).await;
        }

        NetworkMessage::Previous(song) => {
            println!("[syncho] Skipping to previous song: {} — {}", song.song_name, song.artist_name);
            ps.play(song).await;
        }

        NetworkMessage::PlayLater(song)=> {
            println!("[syncho] Queue later: {}", song.song_name);
            ps.play_later(song).await;
        }

        NetworkMessage::PlayNext(song)=> {
            println!("[syncho] + Play next: {}", song.song_name);
            ps.play_next(song).await;
        }

        // Auth messages shouldn't arrive here
        NetworkMessage::Auth { .. } | NetworkMessage::AuthOk | NetworkMessage::AuthFail => {}
    }
}
