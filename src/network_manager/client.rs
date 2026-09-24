use super::types::*;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpStream};
use std::sync::{Arc};
use futures::lock::Mutex;
use crate::player::player::PlayState;

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
