use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use easy_repl::anyhow;
use mini_async_repl::{
    command::{Command, CommandArgInfo, ExecuteCommand},
    CommandStatus, Repl,
};

use crate::helpers::get_input;
use crate::network_manager::{HostBroadcaster, NetworkMessage};
use crate::player::player::PlayState;
use crate::player::types::Song;

fn get_song() -> Song {
    let song_name = get_input(&"Song name".to_string(), false);
    let album_name = get_input(&"Album Name".to_string(), true);
    let artist_name = get_input(&"Artist Name".to_string(), true);

    Song { song_name, artist_name, album_name, position: 0 }
}

// --- "code" command: just prints the session code ---
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

// "pp" command: play-pause the player
struct PlayPauseHandler {
    play_state: Arc<Mutex<PlayState>>,
    broadcaster: HostBroadcaster,
}

impl PlayPauseHandler {
    async fn handle_command(&mut self) -> anyhow::Result<CommandStatus> {
        let play_state = Arc::clone(&self.play_state);
        play_state.lock().unwrap().play_pause().await;
        self.broadcaster.broadcast(NetworkMessage::PlayPause);
        Ok(CommandStatus::Done)
    }
}

impl ExecuteCommand for PlayPauseHandler {
    fn execute(
        &mut self,
        _args: Vec<String>,
        _args_info: Vec<CommandArgInfo>,
    ) -> Pin<Box<dyn Future<Output = anyhow::Result<CommandStatus>> + '_>> {
        Box::pin(self.handle_command())
    }
}

// --- "ps" command: play a specific song ---
struct PlaySongHandler {
    play_state: Arc<Mutex<PlayState>>,
    broadcaster: HostBroadcaster,
}

impl PlaySongHandler {
    async fn handle_command(&mut self) -> anyhow::Result<CommandStatus> {
        let song = get_song();
        self.broadcaster.broadcast(NetworkMessage::Play(song.clone()));
        Arc::clone(&self.play_state).lock().unwrap().play(song).await;
        Ok(CommandStatus::Done)
    }
}
impl ExecuteCommand for PlaySongHandler {
    fn execute(
        &mut self,
        _args: Vec<String>,
        _args_info: Vec<CommandArgInfo>,
    ) -> Pin<Box<dyn Future<Output = anyhow::Result<CommandStatus>> + '_>> {
        Box::pin(self.handle_command())
    }
}
// --

// --- "pl" command: play a specific song later
struct PlayLaterHandler {
    play_state: Arc<Mutex<PlayState>>,
    broadcaster: HostBroadcaster,
}

impl PlayLaterHandler {
    async fn handle_command(&mut self) -> anyhow::Result<CommandStatus> {
        let song = get_song();
        self.broadcaster.broadcast(NetworkMessage::PlayLater(song.clone()));
        Arc::clone(&self.play_state).lock().unwrap().play_later(song).await;
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

// -- previous command: Play a previous song
struct PlayPreviousHandler {
    play_state: Arc<Mutex<PlayState>>,
    broadcaster: HostBroadcaster,
}

impl PlayPreviousHandler {
    async fn handle_command(&mut self) -> anyhow::Result<CommandStatus> {
        let play_state = Arc::clone(&self.play_state);
        play_state.lock().unwrap().previous().await;
        
        tokio::time::sleep(std::time::Duration::from_millis(800)).await;

        let mut curr_song = play_state.lock().unwrap().get_current_song().await;
        if curr_song.is_none() {
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
            curr_song = play_state.lock().unwrap().get_current_song().await;
        }

        if let Some(song) = curr_song {
            println!("[syncho] Playing previous song: {} — {}", song.song_name, song.artist_name);
            self.broadcaster.broadcast(NetworkMessage::Previous(song.clone()));
        } else {
            println!("[syncho] Skipped to previous track (no track details returned).");
        }
        Ok(CommandStatus::Done)
    }
}

impl ExecuteCommand for PlayPreviousHandler {
    fn execute(
        &mut self,
        _args: Vec<String>,
        _args_info: Vec<CommandArgInfo>,
    ) -> Pin<Box<dyn Future<Output = anyhow::Result<CommandStatus>> + '_>> {
        Box::pin(self.handle_command())
    }
}
// --

// -- next command: Skip song to next one
struct NextHandler {
    play_state: Arc<Mutex<PlayState>>,
    broadcaster: HostBroadcaster,
}

impl NextHandler {
    async fn handle_command(&mut self) -> anyhow::Result<CommandStatus> {
        let play_state = Arc::clone(&self.play_state);
        play_state.lock().unwrap().next().await;
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        let curr_song = play_state.lock().unwrap().get_current_song().await;
        // if curr_song.is_none() {
        //     tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        //     curr_song = play_state.lock().unwrap().get_current_song().await;
        // }

        if let Some(song) = curr_song {
            println!("[syncho] Playing next song: {} — {}", &song.song_name, &song.artist_name);
            self.broadcaster.broadcast(NetworkMessage::Next(song.clone()));
        } else {
            println!("[syncho] Skipped to next track (no track details returned).");
        }
        Ok(CommandStatus::Done)
    }
}

impl ExecuteCommand for NextHandler {
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
    broadcaster: HostBroadcaster,
}

impl PlayNextHandler {
    async fn handle_command(&mut self) -> anyhow::Result<CommandStatus> {
        let song = get_song();
        self.broadcaster.broadcast(NetworkMessage::PlayNext(song.clone()));
        let play_state = Arc::clone(&self.play_state);
        play_state.lock().unwrap().play_next(song).await;
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

pub async fn looper(code: String, play_state: Arc<PlayState>, broadcaster: HostBroadcaster) {
    let ps = Arc::into_inner(play_state).unwrap();
    let play_state = Arc::new(Mutex::new(ps));
    let mut repl = Repl::builder()
        .add("code", Command::new(
            "Print the code",
            vec![],
            Box::new(CodeHandler {
                code: code,
            })
        ))
        .add("ps", Command::new(
            "Play a specific song",
            vec![],
            Box::new(PlaySongHandler {
                play_state: Arc::clone(&play_state),
                broadcaster: broadcaster.clone(),
            }),
        ))
        .add("pp", Command::new(
            "Play-Pause the player",
            vec![],
            Box::new(PlayPauseHandler {
                play_state: Arc::clone(&play_state),
                broadcaster: broadcaster.clone(),
            })
        ))
        .add("pl", Command::new(
            "Queue a song to play later (end of queue)",
            vec![],
            Box::new(PlayLaterHandler {
                play_state: Arc::clone(&play_state),
                broadcaster: broadcaster.clone(),
            })
        ))
        .add("previous", Command::new(
            "Play previous song",
            vec![],
            Box::new(PlayPreviousHandler {
                play_state: Arc::clone(&play_state),
                broadcaster: broadcaster.clone(),
            })
        ))
        .add("next", Command::new(
            "Skip to next song",
            vec![],
            Box::new(NextHandler {
                play_state: Arc::clone(&play_state),
                broadcaster: broadcaster.clone(),
            })
        ))
        .add("pn", Command::new(
            "Play a specific song next",
            vec![],
            Box::new(PlayNextHandler {
                play_state: Arc::clone(&play_state),
                broadcaster: broadcaster.clone(),
            })
        ))
        .build()
        .expect("Failed to create repl");

    repl.run().await.expect("REPL error");
}