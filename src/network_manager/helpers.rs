use super::*;
use crate::helpers::format_song;
use crate::player::types::PlayState;
use tokio::sync::{Mutex};
use std::sync::Arc;

/// Writes a `NetworkMessage` into the `SendStream`
pub async fn send(
    writer: &mut iroh::endpoint::SendStream,
    msg: NetworkMessage,
) -> anyhow::Result<()> {
    let payload = msg.to_bytes();
    let size = payload.len() as u32;
    
    println!("Writing size: {}", size);
    writer.write_all(&size.to_be_bytes()).await?; // Write the message size first
    writer.write_all(&payload).await?; // Write the actual message

    Ok(())
}

/// Reads a `NetworkMessage` from the `RecvStream`
pub async fn read(recv: &mut iroh::endpoint::RecvStream) -> anyhow::Result<NetworkMessage> {
    let mut len = [0u8; 4];
    recv.read_exact(&mut len).await?; // Get the message size first
    
    let len = u32::from_be_bytes(len) as usize;
    println!("Reading size - {}", len);
    anyhow::ensure!(len <= 2000, "frame too large");
    
    let mut buf = vec![0u8; len];
    recv.read_exact(&mut buf).await?; // Read the actual message
    Ok(NetworkMessage::from_bytes(&buf))
}

/// Process the receieved event and return if the connection needs to be kept alive 
/// (kinda dumb for every NetworkMessage but it is what it is)
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

        NetworkMessage::Resume => {
            println!("[syncho] Host resume");
            ps.resume().await;
            true
        }

        NetworkMessage::Pause => {
            println!("[syncho] Host pause");
            ps.pause().await;
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
            let _ = writer.unwrap().finish();
            false
        }

        _ => {true}
    }
}
