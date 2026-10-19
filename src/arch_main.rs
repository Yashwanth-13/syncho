mod config_manager;
mod helpers;
mod network_manager;
mod player;
mod repl;

use crate::{
    helpers::{generate_numeric_code, get_input}, network_manager::{
        client::join_session,
        host::{start_host},
    }, player::types::{Config, PlayState}, repl::Role,
};

use clap::Parser;
use futures::lock::Mutex;
use std::sync::Arc;
use tokio::net::TcpListener;

#[derive(Parser, Debug)]
enum Command {
    /// Host a jam
    Host,
    /// Join a jam from a ticket
    Join {
        /// The ticket, as base32 string.
        ticket: String,
    },
}

#[tokio::main]
async fn main() {
    let config = Config::get_config();
    let play_state: Arc<Mutex<PlayState>> = Arc::new(Mutex::new(PlayState::new(&config).await));

    let command = Command::parse();

    match command {
        Command::Host => {
            let code = Arc::new(generate_numeric_code());
            println!("Session code: {}", code);

            let listener = TcpListener::bind("0.0.0.0:8080").await.unwrap();
            println!("Listening on {}", listener.local_addr().unwrap());

            start_host(listener, play_state.clone(), Arc::clone(&code)).await;

            repl::looper(code.to_string(), play_state, Role::Host).await;
        },

        Command::Join { ticket } => {
            let host_addr = format!("{}:8080", get_input(&"Host IP".to_string(), false));
            // let host_addr = "127.0.0.1:8080";
            let code = get_input(&"Session Code".to_string(), false);

            let result = join_session(&host_addr, &code, play_state.clone()).await;

            match result {
                Ok(writer) => {
                    repl::looper(code.to_string(), play_state, Role::Client { writer }).await;
                }

                Err(_) => {
                    eprintln!("[syncho] Failed to connect to host")
                }
            };
        }
    }
}
