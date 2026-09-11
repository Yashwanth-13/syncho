mod network_manager;
mod config_manager;
mod helpers;
mod player;

use std::{sync::Arc, time::{SystemTime}};

use easy_repl::{Repl, CommandStatus, command};
use tokio::{net::{TcpListener, TcpStream}, sync::broadcast};

use clap::{Parser};
use crate::{helpers::{generate_numeric_code, get_input}};

#[derive(Parser, Debug)]
#[command(author, version, about)]
struct Args {

    #[arg(long = "host")]
    host: bool,

    #[arg(long = "join")]
    join: bool,
}

async fn looper(code: String) {
    let mut repl = Repl::builder()
        .add("code", command! {
            "Get the session code",
            () => || {
                println!("Session Code: {}", &code);
                Ok(CommandStatus::Done)
            }
        })
        .add("pp", command! {
            "Play-Pause the song",
            () => || {
                // TODO: Network Handle for Play-Pause
                Ok(CommandStatus::Done)
            }
        })
        .build().expect("Failed to create repl");

    repl.run().expect("REPL error");
}

#[tokio::main]
async fn main() {
    let config = config_manager::get_config();

    // TODO - Get a PlayerState object using config



    let args = Args::parse();
    if args.host {
        let code = Arc::new(generate_numeric_code());
        println!("{}", code);
        tokio::spawn(looper(code.clone().to_string()));
        
        let listener = TcpListener::bind("127.0.0.1:6364").await.unwrap();
        println!("Listening on {}", listener.local_addr().unwrap());

        // let broadcast_channel = broadcast::Sender
        loop {
            let (socket, addr) = listener.accept().await.unwrap();
            println!("New connection from {}", addr);

            tokio::spawn(async move {
                // Process the socket concurrently
                if let Err(e) = network_manager::handle_connection(socket).await {
                    println!("Error handling connection: {}", e);
                }
            });
        }
    } else if args.join {
        let code = get_input(&"Session Code".to_string());
    }
}
