mod config_manager;
mod helpers;
mod network_manager;
mod player;
mod repl;

use crate::{
    helpers::{generate_numeric_code, get_input}, network_manager::{helpers::listen_to_playback, types::{HostBroadcaster, NetworkManager, NetworkMessage}}, player::types::{Config, PlayState}, repl::Role,
};

use clap::Parser;
use std::sync::Arc;
use tokio::sync::{Mutex, broadcast};

#[derive(Parser, Debug)]
enum Command {
    /// Host a jam
    Host,
    /// Join a jam
    Join,
}

#[tokio::main]
async fn main() {
    const ALPN: &[u8] = b"syncho/0";
    let config = Config::get_config();
    let play_state: Arc<Mutex<PlayState>> = Arc::new(Mutex::new(PlayState::new(&config).await));

    let command = Command::parse();
    let endpoint = iroh::Endpoint::bind(iroh::endpoint::presets::N0).await.unwrap();
    match command {
        Command::Host => {
            let code = generate_numeric_code();
            println!("Session code: {}", code);

            let (tx, rx) = broadcast::channel::<NetworkMessage>(64);
            let broadcaster = HostBroadcaster { tx };
            tokio::spawn(listen_to_playback(broadcaster.clone(), Arc::clone(&play_state))); // Task to process playback events
            
            let router = iroh::protocol::Router::builder(endpoint).accept(ALPN, NetworkManager{  code: code.clone(), bc: broadcaster.clone(), play_state: Arc::clone(&play_state) }).spawn();
            
            router.endpoint().online().await;
            println!("Endpoint address - {:?}", router.endpoint().addr());
            println!("Awaiting endpoint online..");
            println!("Done");
            
            repl::looper(code.to_string(), Arc::clone(&play_state), Role::Host).await;
            router.shutdown().await;
        },

        Command::Join => {
            let host_addr = format!("{}", get_input(&"Host IP".to_string(), false));
            let code = get_input(&"Session Code".to_string(), false);

        }
    }
}
