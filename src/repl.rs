use std::future::Future;
use std::pin::Pin;
use futures::lock::Mutex;
use std::sync::{Arc};
use mini_async_repl::{
    command::{Command, CommandArgInfo, ExecuteCommand},
    CommandStatus, Repl,
    anyhow
};

use crate::helpers::get_input;
use crate::network_manager::types::{HostBroadcaster, NetworkMessage};
use crate::player::player::PlayState;
use crate::player::types::Song;

fn get_song() -> Song {
    let song_name = get_input(&"Song name".to_string(), false);
    let artist_name = get_input(&"Artist Name".to_string(), true);
    let album_name = get_input(&"Album Name".to_string(), true);

    Song { song_name, artist_name, album_name, position: 0 }
}

// "code" command: just prints the session code
struct CodeHandler {
    code: String,
}
impl CodeHandler {
    async fn handle_command(&mut self) -> anyhow::Result<CommandStatus> {
        println!("Session Code: {}", &self.code);
        Ok(CommandStatus::Done)
    }
}
impl ExecuteCommand for CodeHandler {
    fn execute(
        &mut self,
        _args: Vec<String>,
        _args_info: Vec<CommandArgInfo>,
    ) -> Pin<Box<dyn Future<Output = anyhow::Result<CommandStatus>> + '_>> {
        Box::pin(self.handle_command())
    }
}

// "pl" command: play a specific song later
struct PlayLaterHandler {
    play_state: Arc<Mutex<PlayState>>,
    broadcaster: Option<HostBroadcaster>,
    is_host: bool
}

impl PlayLaterHandler {
    async fn handle_command(&mut self) -> anyhow::Result<CommandStatus> {
        let song = get_song();
        match self.broadcaster.as_mut() {
            Some(bc) => {bc.broadcast(NetworkMessage::PlayNext(song.clone()));},
            _ => {}
        }
        Arc::clone(&self.play_state).lock().await.play_later(song).await;
        Ok(CommandStatus::Done)
    }
}

impl ExecuteCommand for PlayLaterHandler {
    fn execute(
        &mut self,
        _args: Vec<String>,
        _args_info: Vec<CommandArgInfo>,
    ) -> Pin<Box<dyn Future<Output = anyhow::Result<CommandStatus>> + '_>> {
        Box::pin(self.handle_command())
    }
}
// --


// -- pn command: Play a specific song next
struct PlayNextHandler {
    play_state: Arc<Mutex<PlayState>>,
    broadcaster: Option<HostBroadcaster>,
    is_host: bool
}

impl PlayNextHandler {
    async fn handle_command(&mut self) -> anyhow::Result<CommandStatus> {
        let song = get_song();
        match self.broadcaster.as_mut() {
            Some(bc) => {bc.broadcast(NetworkMessage::PlayNext(song.clone()));},
            _ => {}
        }
        let play_state = Arc::clone(&self.play_state);
        play_state.lock().await.play_next(song).await;
        Ok(CommandStatus::Done)
    }
}

impl ExecuteCommand for PlayNextHandler {
    fn execute(
        &mut self,
        _args: Vec<String>,
        _args_info: Vec<CommandArgInfo>,
    ) -> Pin<Box<dyn Future<Output = anyhow::Result<CommandStatus>> + '_>> {
        Box::pin(self.handle_command())
    }
}
// --

pub async fn looper(code: String, play_state: Arc<Mutex<PlayState>>, broadcaster: Option<HostBroadcaster>, is_host: bool) {
    let mut repl = Repl::builder()
        .add("code", Command::new(
            "Print the code",
            vec![],
            Box::new(CodeHandler {
                code: code,
            })
        ))
        .add("pn", Command::new(
            "Play a specific song next",
            vec![],
            Box::new(PlayNextHandler {
                play_state: Arc::clone(&play_state),
                broadcaster: broadcaster.clone(),
                is_host: is_host.clone()
            })
        ))
        .add("pl", Command::new(
            "Queue a song to play later (end of queue)",
            vec![],
            Box::new(PlayLaterHandler {
                play_state: Arc::clone(&play_state),
                broadcaster: broadcaster.clone(),
                is_host: is_host.clone()
            })
        ))
        .build()
        .expect("Failed to create repl");

    repl.run().await.expect("REPL error");
}