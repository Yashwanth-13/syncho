use tokio::sync::Mutex;
use mini_async_repl::{
    CommandStatus, Repl, anyhow,
    command::{Command, CommandArgInfo, ExecuteCommand},
};
use std::{future::Future};
use std::pin::Pin;
use std::sync::Arc;

use crate::{helpers::{get_input}, player::types::PlayState};
use crate::network_manager::{NetworkMessage, helpers::send};
use crate::player::types::Song;

fn get_song() -> Song {
    let song_name = get_input(&"Song name".to_string(), false);
    let artist_name = get_input(&"Artist Name".to_string(), true);
    let album_name = get_input(&"Album Name".to_string(), true);

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
    Client { connection: iroh::endpoint::Connection },
}

struct FnCmd<F>(F);
impl<F, Fut> ExecuteCommand for FnCmd<F>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = anyhow::Result<CommandStatus>> + 'static,
{
    fn execute(
        &mut self,
        _args: Vec<String>,
        _info: Vec<CommandArgInfo>,
    ) -> Pin<Box<dyn Future<Output = anyhow::Result<CommandStatus>> + '_>> {
        Box::pin((self.0)())
    }
}

fn cmd<F, Fut>(desc: &str, f: F) -> Command
where
    F: FnMut() -> Fut + 'static,
    Fut: Future<Output = anyhow::Result<CommandStatus>> + 'static,
{
    Command::new(desc, vec![], Box::new(FnCmd(f)))
}

pub async fn looper(code: String, ps: Arc<Mutex<PlayState>>, role: Role) -> anyhow::Result<()> {
    let pn = {
        let (ps, role) = (ps.clone(), role.clone());
        cmd("Play a song next (if you are the listener, request the host to play a song next instead)", move || {
            let (ps, role) = (ps.clone(), role.clone());
            async move {
                let song = tokio::task::block_in_place(get_song);
                match role {
                    Role::Host { .. } => ps.lock().await.play_next(song).await,
                    Role::Client { connection } => {
                        let (mut writer, reader) = connection.open_bi().await.unwrap();
                        let _ = send(&mut writer, NetworkMessage::PlayNext(song)).await;
                        writer.finish()?;
                    }
                }
                Ok(CommandStatus::Done)
            }
        })
    };

    let queue = {
        let (ps, role) = (ps.clone(), role.clone());
        cmd("Get the song queue (if you are the listener, it wil be the host's queue)", move || {
            let (ps, role) = (ps.clone(), role.clone());
            async move {
                match role {
                    Role::Host { .. } => {
                        for (i, s) in ps.lock().await.get_queue().await.iter().enumerate() {
                            println!("{} - {s}", i + 1);
                        }
                    }
                    Role::Client { connection } => {
                        let (mut writer, reader) = connection.open_bi().await.unwrap();
                        let _ = send(&mut writer, NetworkMessage::GetQueue).await;
                        writer.finish()?;
                    }
                }
                Ok(CommandStatus::Done)
            }
        })
    };

    let mut repl = Repl::builder()
        .add("code", cmd("Print the code", move || {
            let code = code.clone();
            async move { println!("Session Code: {code}"); Ok(CommandStatus::Done) }
        }))
        .add("play-next", pn)
        .add("get-queue", queue)
        .build()
        .expect("Failed to create repl")
        .run().await?;

    Ok(())
}