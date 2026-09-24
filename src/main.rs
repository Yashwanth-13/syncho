mod network_manager;
mod config_manager;
mod helpers;
mod player;
mod repl;
mod media_listener;

use crate::{
    helpers::{generate_numeric_code, get_input}, network_manager::{client::join_session, host::{listen_to_playback, start_host}}, player::types::{Config, PlayState},
};

use futures::lock::Mutex;
use std::sync::{Arc};
use tokio::{net::TcpListener};
use clap::Parser;

#[derive(Parser, Debug)]
#[command(author, version, about)]
struct Args {
    #[arg(long = "host")]
    host: bool,

    #[arg(long = "join")]
    join: bool,
}

#[tokio::main]
async fn main() {
    let config = Config::get_config();
    let play_state: Arc<Mutex<PlayState>> = Arc::new(Mutex::new(PlayState::new(&config)));

    let args = Args::parse();
    if args.host {
        let code = Arc::new(generate_numeric_code());
        println!("Session code: {}", code);

        let listener = TcpListener::bind("0.0.0.0:8080").await.unwrap();
        println!("Listening on {}", listener.local_addr().unwrap());

        let curr_player = play_state.lock().await.get_player_str();
        let broadcaster = start_host(listener, play_state.clone(), Arc::clone(&code)).await;

        tokio::spawn(listen_to_playback(broadcaster.clone(), curr_player.clone())); // OS-independent to listen to playback changes
        repl::looper(code.to_string(), play_state, broadcaster).await;

    } else if args.join {
        let host_addr = format!("{}:8080", get_input(&"Host IP".to_string(), false));
        let code = get_input(&"Session Code".to_string(), false);
        
        if let Err(e) = join_session(&host_addr, &code, play_state).await {
            eprintln!("[syncho] Failed to join session: {}", e);
        }
    }
}
