pub mod helpers;

use std::{collections::{HashMap}};
use std::{sync::{Arc}};
use crate::{network_manager::helpers::{process_event, read, send}, player::types::{PlayState, Song}};
use anyhow::{anyhow};
use serde::{Deserialize, Serialize};
use tokio::{sync::{Mutex, broadcast}};
use iroh::{EndpointId, endpoint::Connection, protocol::ProtocolHandler};


#[non_exhaustive]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum NetworkMessage {
    Welcome { name: String},
    LetMeIn { name: String},
    NameErr,
    Bye,
    Seek { position: String },
    GetCurrentState,
    CurrentState { song: Option<Song> },
    Play(Song),
    Resume,
    Pause,
    PlayNext(Song),
    GetQueue,
    Queue { queue: Vec<String> },
}

impl NetworkMessage {

    /// Deserializes a [`NetworkMessage`] from a JSON byte vector with `serde_json`
    pub fn from_bytes(bytes: &[u8]) -> Self {
        serde_json::from_slice(bytes).unwrap()
    }

    /// Serializes a [`NetworkMessage`] into a JSON byte vector with `serde_json`
    pub fn to_bytes(&self) -> Vec<u8> {
        serde_json::to_vec(self).expect("serde_json::to_vec is infallible")
    }
}

#[derive(Debug, Clone)]
pub struct Broadcaster {
    pub tx: broadcast::Sender<NetworkMessage>,
    pub tx_client: broadcast::Sender<ClientOp>
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

#[derive(Debug, Clone)]
pub enum ClientOp {
    Joined { id: EndpointId, name: String, conn: Connection },
    Left { id: EndpointId },
    List,
    Kick { id: EndpointId },
}

/// The `NetworkManager` holds channels to 
#[derive(Debug, Clone)]
pub struct NetworkManager {
    pub name: String,
    pub bc: Broadcaster,
    pub play_state: Arc<Mutex<PlayState>>,
}


impl NetworkManager {

    /// Exchanges names with the new client
    async fn greet( &self, reader: &mut iroh::endpoint::RecvStream, writer: &mut iroh::endpoint::SendStream,curr_song_state: NetworkMessage ) -> Result<String, anyhow::Error> {
        let message = read(reader).await.unwrap();

        match message {
            NetworkMessage::LetMeIn { name } => {
                send(writer, NetworkMessage::Welcome { name: self.name.clone() }).await?;
                send(writer, curr_song_state).await?;
                return Ok(name);
            },

            _ => {
                return Err(anyhow!("No name"));
            }
        };
        
    }

    /// Listener to client's NetworkMessages
    async fn listen_to_client(&self, connection: &Connection) -> anyhow::Result<()> {
        loop {
            let (mut command_write, mut command_read) = connection.accept_bi().await?;
            let command = read(&mut command_read).await.unwrap();
            // println!("{:?}", command);
            if !process_event(command, &self.play_state, Some(&mut command_write)).await {
                break;
            }
        }
        Ok(())
    }

    /// Listener to broadcasted playback events
    async fn forward_events(&self, connection: &Connection) -> anyhow::Result<()>{
        let mut receiver = self.bc.tx.subscribe();
        while let Ok(msg) = receiver.recv().await {
            println!("Got a bc msg: {:?}", msg);
            let mut writer = connection.open_uni().await.unwrap();
            send(&mut writer, msg.clone()).await?;
            writer.stopped();
            writer.finish()?;
            match msg {
                NetworkMessage::Bye => {
                    connection.closed().await;
                    break;
                },

                _ => {}
            }
        }

        Ok(())
    }

    /// Listener to host's NetworkMessages
    pub async fn listen_events(connection: Connection, play_state: Arc<Mutex<PlayState>>) -> anyhow::Result<(u8)>{
        println!("Listening to host events");
        loop {
            let mut recv = connection.accept_uni().await.unwrap();
            let message = read(&mut recv).await?;
            println!("Got message - {:?}", message);
            if !process_event(message, &play_state, None).await {
                break;
            }
            recv.stop(1u8.into())?;
        }

        Ok(2)
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
            greet_write.finish()?;
        } else {
            connection.close(1u8.into(), b"client-no-name");
            return Ok(());
        }

        tokio::select! {
            _ = self.forward_events(&connection) => {}
            _ = self.listen_to_client(&connection) => {
                connection.close(1u8.into(), b"bye");
                self.bc.send_client_event(ClientOp::Left { id: connection.remote_id() });
            },
        }


        Ok(())
    }
}

pub async fn handle_clients(mut rx: broadcast::Receiver<ClientOp>) {
    let mut clients = HashMap::new();
    let mut used_names = HashMap::<String, u32>::new();
    loop {
        if let Ok(msg) = rx.recv().await {
            match msg {
                ClientOp::Joined { id, name, conn } => { 
                    let name_counter = match used_names.get(&name) {
                        Some(curr_val) => curr_val.to_owned() + 1,
                        None => 1,
                    };

                    used_names.insert(name.clone(), name_counter);
                    clients.insert(id, Client { name, conn });
                }
                ClientOp::Left { id } => {
                    if let Some(client) = clients.remove(&id) {
                        println!("[syncho] {} left", client.name);
                    }

                }
                ClientOp::List => {
                    let names: Vec<(EndpointId, String)> = clients
                        .iter()
                        .map(|(id, c)| (*id, c.name.clone()))
                        .collect();
                    println!("{:?}", names);
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
            return;
        }
    };
}

#[derive(Debug, Clone)]
struct Client {
    name: String,
    conn: Connection
}