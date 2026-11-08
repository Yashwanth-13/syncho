mod config_manager;
mod helpers;
mod network_manager;
mod player;
mod repl;

use crate::{
    helpers::get_input, network_manager::{Broadcaster, ClientOp, NetworkManager, NetworkMessage, helpers::{listen_to_playback, read, send}}, player::types::{Config, PlayState}, repl::{Role, looper},
};

use iroh::{Endpoint, PublicKey};
use serde::{Deserialize, Serialize};
use clap::{Parser};
use core::fmt;
use std::{collections::{HashMap, HashSet}, str::FromStr, sync::Arc};
use tokio::{sync::{Mutex, broadcast}};

#[derive(Parser, Debug)]
enum Command {
    /// Host a jam
    Host,
    /// Join a jam
    Join,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    const ALPN: &[u8] = b"syncho/0";
    let config = Config::get_config();
    let username = Config::get_username(&config);
    let play_state: Arc<Mutex<PlayState>> = Arc::new(Mutex::new(PlayState::new(&config).await));

    let command = Command::parse();
    let endpoint = iroh::Endpoint::bind(iroh::endpoint::presets::N0).await.unwrap();
    match command {
        Command::Host => {
            let (tx, _) = broadcast::channel::<NetworkMessage>(64);
            let (tx_client, rx_clients) = broadcast::channel::<ClientOp>(5);
            let broadcaster = Broadcaster { tx, tx_client };
            tokio::spawn(listen_to_playback(broadcaster.clone(), Arc::clone(&play_state))); // Task to process playback events
            
            // Task to handle client list
            tokio::spawn(network_manager::handle_clients(rx_clients));

            let router = iroh::protocol::Router::builder(endpoint)
            .accept(ALPN, 
                NetworkManager{  
                    name: username.clone(), 
                    bc: broadcaster.clone(), 
                    play_state: Arc::clone(&play_state)
                })
            .spawn();
            
            router.endpoint().online().await;
            let endpoint_addr =  router.endpoint().addr();
            
            let ticket = {
                let id = endpoint_addr.id;
                Ticket { name: username, endpoint_id: id}
            };
            
            let ticket_str = ticket.to_string();
            println!("Ticket: {} - len: {}", ticket, ticket_str.len());

            repl::looper(ticket_str, Arc::clone(&play_state), Role::Host).await?;
            router.shutdown().await?;
        },

        Command::Join => {
            let code = get_input(&"Get the ticket".to_string(), false);
            if let Ok(ticket) = Ticket::from_str(&code) {
                let ep_addr = ticket.endpoint_id;

                let endpoint = Endpoint::bind(iroh::endpoint::presets::N0).await.unwrap();
                let conn = endpoint.connect(ep_addr, ALPN).await.unwrap();

                let (mut writer, mut reader) = conn.open_bi().await.unwrap();
            
                send(&mut writer, NetworkMessage::LetMeIn { name: username }).await.unwrap();
                let host_name = match read(&mut reader).await {
                    Ok(msg) => {
                        match msg {
                            NetworkMessage::Welcome { name } => name,
                            _ => "".to_string(),
                        }
                    },

                    Err(e) => {
                        println!("{}", e);
                        "".to_string()
                    }
                };

                println!("[syncho] Connected to Host: {}", host_name);
                writer.finish()?;

                // Receiver of the final message closes the connection and the sender awaits the closure.
                tokio::select! {
                    _ = looper(code, play_state.clone(), Role::Client { connection: conn.clone() }) => {
                        let (mut writer, mut reader) = conn.open_bi().await.unwrap();
                        send(&mut writer, NetworkMessage::Bye ).await.unwrap();
                        conn.closed().await; // We are the sender here
                    },
                    _ = NetworkManager::listen_events(&conn, &play_state) => {
                        conn.close(1u8.into(), b"host-close"); // We are the receiver here
                    }
                };
            } else {
                println!("no ticket");
            }

        }
    }

    Ok(())
}

#[derive(Debug, Serialize, Deserialize)]
struct Ticket {
    name: String,
    endpoint_id: PublicKey,
    // relay_urls: RelayUrl
}

impl Ticket {
    // Deserialize
    fn from_bytes(bytes: &[u8]) -> Result<Self, anyhow::Error> {
        serde_json::from_slice(bytes).map_err(Into::into)
    }

    // Serialize
    pub fn to_bytes(&self) -> Vec<u8> {
        serde_json::to_vec(self).expect("serde_json::to_vec is infallible")
    }
}

// The `Display` trait allows us to use the `to_string` method on `Ticket`.
impl fmt::Display for Ticket {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut text = data_encoding::BASE32_NOPAD.encode(&self.to_bytes()[..]);
        text.make_ascii_lowercase();
        write!(f, "{}", text)
    }
}

// The `FromStr` trait allows us to turn a `str` into a `Ticket`
impl FromStr for Ticket {
    type Err = anyhow::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let bytes = data_encoding::BASE32_NOPAD.decode(s.to_ascii_uppercase().as_bytes())?;
        Self::from_bytes(&bytes)
    }
}
