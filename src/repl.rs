use tokio::{io::AsyncWriteExt, sync::Mutex};
use mini_async_repl::{
    anyhow,
};
use std::{sync::Arc};
use iroh::endpoint::Connection;

use crate::{helpers::get_input, network_manager::helpers::{read}, player::types::PlayState};
use crate::network_manager::{NetworkMessage, helpers::send};
use crate::player::types::Song;

/// Prompts the user for the details of a song
fn get_song() -> Song {
    let song_name = get_input(&"--> Song name".to_string(), false);
    let artist_name = get_input(&"--> Artist Name".to_string(), true);
    let album_name = get_input(&"--> Album Name".to_string(), true);

    Song {
        song_name,
        artist_name,
        album_name,
        position: 0,
    }
}

#[derive(Clone)]
pub enum Role {
    Host,
    Client { connection: Connection },
}

/// Helper for queue printing
fn print_queue(queue: Vec<String>) {
    if queue.len() == 0 {
        println!("[syncho] Empty queue");
        return;
    }

    for (index, song) in queue.iter().enumerate() {
        println!("{} - {}", index+1, song);
    }
}

/// Gets the song queue. Sends a `NetworkMessage` to host if used as a client; prints locally if used as the host of the session
async fn get_queue(ps: &Arc<Mutex<PlayState>>, role: &Role) -> anyhow::Result<()> {
    let queue = match role {
        Role::Host => {ps.lock().await.get_queue().await},

        Role::Client { connection } => {
            let (mut writer, mut reader) = connection.open_bi().await?;
            let _ = send(&mut writer, NetworkMessage::GetQueue).await;
            writer.finish()?;
            writer.stopped();
            let message = read(&mut reader).await.unwrap();
            let queue = match message {
                NetworkMessage::Queue { queue } => {queue},

                _ => vec![]
            };
            
            queue
        }
    };

    print_queue(queue);

    Ok(())
}

/// Sets a specific song to be played next. Sends a `NetworkMessage` to host if used as a client.
async fn play_next(ps: &Arc<Mutex<PlayState>>, role: &Role) -> anyhow::Result<()> {
    let song = get_song();
    match role {
        Role::Host => {ps.lock().await.play_next(song).await},
        Role::Client { connection } => {
            let (mut writer, _) = connection.open_bi().await?;
            let _ = send(&mut writer, NetworkMessage::PlayNext(song)).await;
            writer.finish()?;
            writer.stopped();
            
        }
    }

    Ok(())
}

/// Starts the REPL for both client and host.
pub async fn looper(code: String, ps: Arc<Mutex<PlayState>>, role: Role) -> anyhow::Result<u8> {
    let queue_getter = ["gq", "get-queue"];
    let next_player = ["pn", "play-next"];

    loop {
        let command = get_input(">", true);

        for item in queue_getter {
            if item.eq(&command) {
                let _ = get_queue(&ps, &role).await;
            }
        }

        for item in next_player {
            if item.eq(&command) {
                let _ = play_next(&ps, &role).await;
            }
        }

        if command.eq("code") {println!("{}", code)};

        if command.eq("exit") || command.eq("quit") {break}
    }

    Ok(1)
}