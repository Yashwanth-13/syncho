use std::sync::Arc;
use std::time::SystemTime;

use serde::{Deserialize, Serialize};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{broadcast, Mutex};

use crate::player::player::PlayState;
use crate::player::types::Song;

// ---------------------------------------------------------------------------
// Wire protocol – newline-delimited JSON
// ---------------------------------------------------------------------------

/// Every message sent over the wire is one of these variants.
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum NetworkMessage {
    /// Client → Host: first message after connect, carries the session code.
    Auth { code: String },

    /// Host → Client: authentication succeeded, you're in.
    AuthOk,

    /// Host → Client: wrong code, connection will be closed.
    AuthFail,

    /// Host → Clients: start playing this song now.
    Play {
        song_name: String,
        artist_name: String,
        album_name: String,
        /// Unix timestamp (secs) when the song started on the host.
        started_at_secs: u64,
    },

    /// Host → Clients: toggle play / pause.
    PlayPause,

    /// Host → Clients: skip to next track.
    Next {
        song_name: String,
        artist_name: String, 
        album_name: String
    },

    /// Host → Clients: go back to previous track.
    Previous {
        song_name: String,
        artist_name: String, 
        album_name: String
    },


    /// Host → Clients: add this song to end of queue.
    PlayLater {
        song_name: String,
        artist_name: String,
        album_name: String,
    },

    /// Host → Clients: queue this song to play after the current one.
    PlayNext {
        song_name: String,
        artist_name: String,
        album_name: String,
    },
}

impl NetworkMessage {
    /// Serialize to a newline-terminated JSON string ready to send over the wire.
    pub fn to_wire(&self) -> String {
        let mut s = serde_json::to_string(self).expect("serialization is infallible");
        s.push('\n');
        s
    }
}

// ---------------------------------------------------------------------------
// Host side
// ---------------------------------------------------------------------------

/// A cloneable handle the REPL uses to broadcast events to every connected client.
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

/// Bind a TCP listener on `0.0.0.0:8080`, spawn a background task that
/// accepts clients and validates them against `session_code`.
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

/// Authenticate a single client then pump broadcast messages to it.
async fn handle_client(
    stream: TcpStream,
    session_code: Arc<String>,
    mut rx: broadcast::Receiver<NetworkMessage>,
) {
    let (read_half, mut write_half) = stream.into_split();
    let mut reader = BufReader::new(read_half);
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
        let _ = write_half
            .write_all(NetworkMessage::AuthFail.to_wire().as_bytes())
            .await;
        println!("[syncho] Client failed authentication.");
        return;
    }

    let _ = write_half
        .write_all(NetworkMessage::AuthOk.to_wire().as_bytes())
        .await;
    println!("[syncho] Client authenticated successfully.");

    // ---- Forward broadcast messages ----
    loop {
        match rx.recv().await {
            Ok(msg) => {
                if write_half
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

// ---------------------------------------------------------------------------
// Client side
// ---------------------------------------------------------------------------

/// Connect to `host_addr` (e.g. `"192.168.1.5:8080"`), authenticate with
/// `session_code`, then listen for events and apply them to `play_state`.
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
    let mut ps = play_state.lock().await;

    match msg {
        NetworkMessage::Play {
            song_name,
            artist_name,
            album_name,
            started_at_secs,
        } => {
            let song = Song {
                song_name,
                artist_name,
                album_name,
                time_started: SystemTime::UNIX_EPOCH
                    + std::time::Duration::from_secs(started_at_secs),
            };
            println!("[syncho] ▶ Playing: {} — {}", song.song_name, song.artist_name);
            ps.play(song).await;
        }

        NetworkMessage::PlayPause => {
            println!("[syncho] ⏯ Play/Pause");
            ps.play_pause().await;
        }

        NetworkMessage::Next {
            song_name,
            artist_name, 
            album_name
        } => {
            let song = Song {
                song_name,
                artist_name,
                album_name,
                time_started: SystemTime::now(),
            };
            ps.play(song).await;
        }

        NetworkMessage::Previous {
            song_name,
            artist_name, 
            album_name
        } => {
            let song = Song {
                song_name,
                artist_name,
                album_name,
                time_started: SystemTime::now(),
            };
            ps.play(song).await;
        }

        NetworkMessage::PlayLater {
            song_name,
            artist_name,
            album_name,
        } => {
            let song = Song {
                song_name,
                artist_name,
                album_name,
                time_started: SystemTime::now(),
            };
            println!("[syncho] + Queue later: {}", song.song_name);
            ps.play_later(song).await;
        }

        NetworkMessage::PlayNext {
            song_name,
            artist_name,
            album_name,
        } => {
            let song = Song {
                song_name,
                artist_name,
                album_name,
                time_started: SystemTime::now(),
            };
            println!("[syncho] + Play next: {}", song.song_name);
            ps.play_next(song).await;
        }

        // Auth messages shouldn't arrive here
        NetworkMessage::Auth { .. } | NetworkMessage::AuthOk | NetworkMessage::AuthFail => {}
    }
}
