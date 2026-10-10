use super::types::*;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, BufWriter};
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
use tokio::net::{TcpStream};
use std::process::exit;
use std::sync::{Arc};
use futures::lock::Mutex;
use crate::network_manager::helpers::{process_event, send};
use crate::player::player::PlayState;

pub async fn join_session(
    host_addr: &str,
    session_code: &str,
    play_state: Arc<Mutex<PlayState>>,
) -> Result<Arc<Mutex<BufWriter<OwnedWriteHalf>>>, Box<dyn std::error::Error>> {
    let stream = TcpStream::connect(host_addr).await?;
    println!("[syncho] Connected to host at {}", host_addr);

    let (read_half, write_half) = stream.into_split();
    let reader: Arc<Mutex<BufReader<OwnedReadHalf>>> = Arc::new(Mutex::new(BufReader::new(read_half)));
    let writer = Arc::new(Mutex::new(BufWriter::new(write_half)));

    // Send auth
    let auth_msg = NetworkMessage::Auth {
        code: session_code.to_string(),
    };

    let _ = send(&writer, auth_msg).await;
    // Read auth response
    let mut line = String::new();
    {reader.lock().await.read_line(&mut line).await?;}
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

    // Listening to host
    tokio::spawn(listen_to_host(line, Arc::clone(&reader), play_state));
    
    Ok(writer)
}

// Continuous loop to listen to host's events and process them
async fn listen_to_host(mut line: String, reader: Arc<Mutex<BufReader<OwnedReadHalf>>>, play_state: Arc<Mutex<PlayState>>) {
    loop {
        line.clear();
        let n = {reader.lock().await.read_line(&mut line).await.unwrap()};
        if n == 0 {
            println!("[syncho] Host closed the connection.");
            exit(0);
        }

        let msg: NetworkMessage = match serde_json::from_str(line.trim()) {
            Ok(m) => m,
            Err(e) => {
                eprintln!("[syncho] Bad message from host: {} — {:?}", e, line);
                line.clear();
                continue;
            }
        };

        process_event(msg, Arc::clone(&play_state), None).await;
    }
}