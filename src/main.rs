mod network_manager;
mod config_manager;
mod helpers;
mod player;
mod repl;

use std::{sync::Arc};


use tokio::{net::TcpListener};

use clap::{Parser};
use crate::{helpers::{generate_numeric_code, get_input}, player::{player::PlayState, types::{Config, Player, Song}}};

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
        println!("{}", code);
        
        let listener = TcpListener::bind("0.0.0.0:8080").await.unwrap();

        // TODO - handle_connection
        
        println!("Listening on {}", listener.local_addr().unwrap());
        repl::looper(code.clone().to_string(), PlayState::new(&config)).await;
    } else if args.join {
        let code = get_input(&"Session Code".to_string(), false);
    }
}
