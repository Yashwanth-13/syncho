mod config_manager;
mod helpers;
mod network_manager;
mod player;
mod repl;

use crate::{
    helpers::{generate_numeric_code, get_input}, 
    network_manager::{helpers::listen_to_playback, HostBroadcaster, NetworkManager, NetworkMessage}, 
    player::types::{Config, PlayState}, 
    repl::Role,
};

use iroh::{Endpoint, EndpointAddr, PublicKey, RelayUrl};
use serde::{Deserialize, Serialize};
use clap::{Parser, ValueHint::Url};
use core::fmt;
use std::{collections::HashMap, str::FromStr, sync::Arc};
use tokio::{io::AsyncWriteExt, sync::{Mutex, broadcast}};

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
    let username = Config::get_username(&config);
    let play_state: Arc<Mutex<PlayState>> = Arc::new(Mutex::new(PlayState::new(&config).await));

    let command = Command::parse();
    let endpoint = iroh::Endpoint::bind(iroh::endpoint::presets::N0).await.unwrap();
    match command {
        Command::Host => {
            // let code = generate_numeric_code();
            // println!("Session code: {}", code);

            let (tx, rx) = broadcast::channel::<NetworkMessage>(64);
            let broadcaster = HostBroadcaster { tx };
            tokio::spawn(listen_to_playback(broadcaster.clone(), Arc::clone(&play_state))); // Task to process playback events
            
            let router = iroh::protocol::Router::builder(endpoint)
            .accept(ALPN, 
                NetworkManager{  
                    name: username.clone(), 
                    bc: broadcaster.clone(), 
                    play_state: Arc::clone(&play_state)
                })
            .spawn();
            
            
            router.endpoint().online().await;
            println!("Endpoint is online..");
            let endpoint_addr =  router.endpoint().addr();
            
            let ticket = {
                let id = endpoint_addr.id;
                let iter = endpoint_addr.relay_urls();
                let mut last = RelayUrl::from_str("https://aps1-1.relay.n0.iroh.link./").unwrap();
                for url in iter {
                    last = url.clone();
                }

                Ticket { name: username, endpoint_id: id, relay_urls: last }
            };
            

            let ticket_str = ticket.to_string();
            println!("Ticket: {} - len: {}", ticket, ticket_str.len());
            
            repl::looper(ticket_str, Arc::clone(&play_state), Role::Host).await;
            router.shutdown().await;
        },

        Command::Join => {
            let code = get_input(&"Get the ticket".to_string(), false);
            if let Ok(ticket) = Ticket::from_str(&code) {
                let ep_addr = ticket.endpoint_id;
                let relay = ticket.relay_urls;
                let preset = iroh_services::preset()
                .relays([
                    relay.to_string(),
                ]).unwrap()
                .build().unwrap();

                let endpoint = Endpoint::bind(preset).await.unwrap();

                println!("Tryna connect");
                let conn = endpoint.connect(ep_addr, ALPN).await.unwrap();

                let (mut send, mut recv) = conn.open_bi().await.unwrap();
                send.write_all(&NetworkMessage::Hello.to_bytes()).await;
                send.flush().await;
            } else {
                println!("no ticket");
            }

        }
    }
}


#[derive(Debug, Serialize, Deserialize)]
struct Ticket {
    name: String,
    endpoint_id: PublicKey,
    relay_urls: RelayUrl
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

// The `Display` trait allows us to use the `to_string`
// method on `Ticket`.
impl fmt::Display for Ticket {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut text = data_encoding::BASE64_NOPAD.encode(&self.to_bytes()[..]);
        text.make_ascii_lowercase();
        write!(f, "{}", text)
    }
}

// The `FromStr` trait allows us to turn a `str` into
// a `Ticket`
impl FromStr for Ticket {
    type Err = anyhow::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let bytes = data_encoding::BASE64_NOPAD.decode(s.to_ascii_uppercase().as_bytes())?;
        Self::from_bytes(&bytes)
    }
}
