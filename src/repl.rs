use futures::lock::Mutex;
use mini_async_repl::{
    CommandStatus, Repl, anyhow,
    command::{Command, CommandArgInfo, ExecuteCommand},
};
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use tokio::io::{AsyncWriteExt, BufWriter};
use tokio::net::tcp::OwnedWriteHalf;

use crate::helpers::get_input;
use crate::network_manager::types::{HostBroadcaster, NetworkMessage};
use crate::player::player::PlayState;
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
    Client { writer: Arc<Mutex<BufWriter<OwnedWriteHalf>>> },
}

async fn send(w: &Mutex<BufWriter<OwnedWriteHalf>>, msg: NetworkMessage) -> std::io::Result<()> {
    let mut w = w.lock().await;
    w.write_all(msg.to_wire().as_bytes()).await?;
    w.flush().await
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

pub async fn looper(code: String, ps: Arc<Mutex<PlayState>>, role: Role) {
    let pn = {
        let (ps, role) = (ps.clone(), role.clone());
        cmd("Play a song next", move || {
            let (ps, role) = (ps.clone(), role.clone());
            async move {
                let song = tokio::task::block_in_place(get_song);
                match role {
                    Role::Host { .. } => ps.lock().await.play_next(song).await,
                    Role::Client { writer } => {
                        let _ = send(&writer, NetworkMessage::PlayNext(song)).await;
                    }
                }
                Ok(CommandStatus::Done)
            }
        })
    };

    let queue = {
        let (ps, role) = (ps.clone(), role.clone());
        cmd("Get the host's song queue", move || {
            let (ps, role) = (ps.clone(), role.clone());
            async move {
                match role {
                    Role::Host { .. } => {
                        for (i, s) in ps.lock().await.get_queue().await.iter().enumerate() {
                            println!("{} - {s}", i + 1);
                        }
                    }
                    Role::Client { writer } => {
                        let _ = send(&writer, NetworkMessage::GetQueue).await;
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
        .add("pn", pn)
        .add("get-queue", queue)
        .build()
        .expect("Failed to create repl");

    repl.run().await.expect("REPL error");
}