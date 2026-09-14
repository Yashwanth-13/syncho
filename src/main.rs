mod network_manager;
mod config_manager;
mod helpers;
mod player;
mod repl;

use std::sync::Arc;

use tokio::{net::TcpListener, sync::Mutex};

use clap::Parser;
use crate::{
    helpers::{generate_numeric_code, get_input},
    network_manager::{join_session, start_host},
    player::{player::PlayState, types::Config},
};

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

    let args = Args::parse();
    if args.host {
        let code = Arc::new(generate_numeric_code());
        println!("Session code: {}", code);

        let listener = TcpListener::bind("0.0.0.0:8080").await.unwrap();
        println!("Listening on {}", listener.local_addr().unwrap());

        let broadcaster = start_host(listener, Arc::clone(&code)).await;

        repl::looper(code.to_string(), PlayState::new(&config), broadcaster).await;
    } else if args.join {
        let code = get_input(&"Session Code".to_string(), false);
        let host_addr = format!("{}:8080", get_input(&"Host IP:Port (e.g. 192.168.1.5)".to_string(), false));

        let play_state = Arc::new(Mutex::new(PlayState::new(&config)));

        if let Err(e) = join_session(&host_addr, &code, play_state).await {
            eprintln!("[syncho] Failed to join session: {}", e);
        }
    }
}
