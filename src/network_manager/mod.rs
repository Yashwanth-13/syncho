// pub mod client;
// pub mod host;
pub mod helpers;

use std::{collections::{HashMap, HashSet}};
use std::{sync::{Arc}, time::Duration};
use crate::{network_manager::helpers::{process_event, read, send}, player::types::{PlayState, Song}};
use anyhow::{anyhow};
use serde::{Deserialize, Serialize};
use tokio::{io::AsyncWriteExt, sync::{Mutex, broadcast, mpsc::{self, Receiver}, oneshot}};
use iroh::{EndpointId, endpoint::Connection, protocol::ProtocolHandler};


#[non_exhaustive]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum NetworkMessage {
    Welcome { name: String},
    LetMeIn { name: String},
    NameErr,
    Bye,
    Message { msg: String },
    Seek { position: String },
    GetCurrentState,
    CurrentState { song: Option<Song> },
    Play(Song),
    PlayPause,
    PlayNext(Song),
    GetQueue,
    Queue { queue: Vec<String> },
}

impl NetworkMessage {
    pub fn to_wire(&self) -> String {
        let mut s = serde_json::to_string(self).expect("serialization is infallible");
        s.push('\n');
        s
    }

    pub fn from_bytes(bytes: &[u8]) -> Self {
        serde_json::from_slice(bytes).unwrap()
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        serde_json::to_vec(self).expect("serde_json::to_vec is infallible")
    }
}

#[derive(Debug, Clone)]
pub struct Broadcaster {
    pub tx: broadcast::Sender<NetworkMessage>,
    pub tx_client: mpsc::Sender<ClientOp>
}

impl Broadcaster {
    /// Sends a `NetworkMessage` to the broadcast
    pub fn broadcast(&self, msg: NetworkMessage) {
        // `send` only fails when there are zero receivers, which is fine.
        let _ = self.tx.send(msg);
    }

    pub fn send_client_event(&self, msg: ClientOp) {
        // `send` only fails when there are zero receivers, which is fine.
        let _ = self.tx_client.send(msg);
    }
}

#[derive(Debug)]
pub enum ClientOp {
    Joined { id: EndpointId, name: String, conn: Connection },
    Left { id: EndpointId },
    List { reply: oneshot::Sender<Vec<(EndpointId, String)>> },
    Kick { id: EndpointId },
}


#[derive(Debug, Clone)]
pub struct NetworkManager {
    pub name: String,
    pub bc: Broadcaster,
    pub play_state: Arc<Mutex<PlayState>>,
}


impl NetworkManager {
    async fn connect() {
        todo!();
    }

    async fn greet( &self, reader: &mut iroh::endpoint::RecvStream, writer: &mut iroh::endpoint::SendStream,curr_song_state: NetworkMessage ) -> Result<String, anyhow::Error> {
        let message = read(reader).await.unwrap();

        match message {
            NetworkMessage::LetMeIn { name } => {
                // println!("[syncho] {} joined", name);
                send(writer, NetworkMessage::Welcome { name: self.name.clone() }).await?;
                send(writer, curr_song_state.clone()).await?;
                return Ok(name);
            },

            _ => {
                return Err(anyhow!("No name"));
            }
        };
        
    }
}


impl ProtocolHandler for NetworkManager {
    async fn accept(
        &self,
        connection: iroh::endpoint::Connection,
    ) -> Result<(), iroh::protocol::AcceptError>
    {     
        let mut username = String::new();
        // Send the current song
        let curr_song = { self.play_state.lock().await.get_current_song().await };
        let message = match curr_song {
            Some(song) => NetworkMessage::CurrentState { song: Some(song) },
            None => NetworkMessage::CurrentState { song: None },
        };

        let (mut greet_write, mut greet_read) = connection.accept_bi().await?;
        let result = self.greet(&mut greet_read, &mut greet_write, message).await;
        if let Ok(client_name) = result {
            username.push_str(&client_name);
            println!("[syncho] {} joined", username);
            self.bc.send_client_event(ClientOp::Joined { id: connection.remote_id(), name: client_name, conn: connection.clone() });
        } else {
            connection.close(1u8.into(), b"Client did not send name");
            return Ok(());
        }

        connection.closed().await;
        self.bc.send_client_event(ClientOp::Left { id: connection.remote_id() });
        Ok(())
    }
}

impl NetworkManager {
    // Listen to messages from clients
    async fn read_client_stream(
        client: String,
        reader: Arc<Mutex<iroh::endpoint::RecvStream>>,
        writer: Arc<Mutex<iroh::endpoint::SendStream>>,
        play_state: Arc<Mutex<PlayState>>,
    ) {
        let mut line = String::new();
        loop {
            line.clear();
            let incoming_data = { reader.lock().await.read_to_end(1000).await };

            match incoming_data {
                Ok(read_bytes) => {
                    let msg: NetworkMessage = NetworkMessage::from_bytes(&read_bytes);
                    process_event(msg, &play_state, Some(&writer)).await;
                },
                Err(e) => {
                    eprintln!("[syncho] Bad message from client({}): {}", client, e);
                }
            }
        }
    }
}


pub async fn handle_clients(mut rx: Receiver<ClientOp>) {
    println!("task started");
    let mut clients = HashMap::new();
    let mut used_names = HashMap::<String, u32>::new();
    loop {
        if let Some(msg) = rx.recv().await {
            match msg {
                ClientOp::Joined { id, name, conn } => { 
                    let name_counter = match used_names.get(&name) {
                        Some(curr_val) => curr_val.to_owned() + 1,
                        None => 1,
                    };

                    println!("Added {}", name.clone());
                    used_names.insert(name.clone(), name_counter);
                    clients.insert(id, Client { name, conn });
                }
                ClientOp::Left { id } => { 
                    if let Some(client) = clients.remove(&id) {
                        println!("[syncho] {} left", client.name);
                    }

                }
                ClientOp::List { reply } => {
                    let names: Vec<(EndpointId, String)> = clients
                        .iter()
                        .map(|(id, c)| (*id, c.name.clone()))
                        .collect();
                    let _ = reply.send(names);
                }
                ClientOp::Kick { id } => {
                    if let Some(client) = clients.remove(&id) {
                        client.conn.close(1u8.into(), b"kicked");
                        match used_names.get(&client.name) {
                            Some(curr_val) => {used_names.insert(client.name, curr_val.to_owned() - 1);},
                            None => {},
                        };
                    }

                }
            }
        } else {
            println!("task dropped");
            return;
        }
    };
}

#[derive(Debug, Clone)]
struct Client {
    name: String,
    conn: Connection
}