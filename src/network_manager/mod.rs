// pub mod client;
// pub mod host;
pub mod helpers;

use std::{collections::HashMap, sync::Arc, time::Duration};
use crate::{network_manager::helpers::{process_event, send}, player::types::{PlayState, Song}};
use anyhow::{anyhow};
use serde::{Deserialize, Serialize};
use tokio::{io::AsyncWriteExt, sync::{Mutex, broadcast}, time::timeout};
use iroh::{protocol::ProtocolHandler};


#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum NetworkMessage {
    Hello,
    Welcome { name: String},
    Thanks { name: String},
    Bye,
    Auth { code: String },
    AuthOk,
    AuthFail,
    Message { msg: String },
    Seek { position: String },
    CurrentState { song: Option<Song> },
    Play(Song),
    PlayPause,
    // PlayLater(Song),
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

    fn from_bytes(bytes: &[u8]) -> Self {
        serde_json::from_slice(bytes).unwrap()
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        serde_json::to_vec(self).expect("serde_json::to_vec is infallible")
    }
}

#[derive(Debug, Clone)]
pub struct HostBroadcaster {
    pub tx: broadcast::Sender<NetworkMessage>,
}

impl HostBroadcaster {
    pub fn broadcast(&self, msg: NetworkMessage) {
        // `send` only fails when there are zero receivers, which is fine.
        let _ = self.tx.send(msg);
    }

    pub fn get_handle(&self) -> broadcast::Receiver<NetworkMessage> {
        self.tx.subscribe()
    }
}

#[derive(Debug, Clone)]
pub struct NetworkManager {
    pub name: String,
    pub bc: HostBroadcaster,
    pub play_state: Arc<Mutex<PlayState>>,
}


impl NetworkManager {
    async fn connect() {
        todo!();
    }

    async fn greet( &self, reader: &mut iroh::endpoint::RecvStream, writer: &mut iroh::endpoint::SendStream ) -> Result<String, anyhow::Error> {

        let message = NetworkMessage::from_bytes(&reader.read_to_end(1000).await?);
        match message {
            NetworkMessage::Hello => {
                println!("Got hello");
                writer.write_all(&NetworkMessage::Welcome { name: self.name.clone() }.to_bytes()).await;
                writer.flush().await;
            },

            _ => {}
        };

        let message = NetworkMessage::from_bytes(&reader.read_to_end(1000).await?);
        match message {
            NetworkMessage::Thanks { name } => {
                println!("Got some name");
                return Ok(name);
            },

            _ => Err(anyhow!("Client did not send name"))
        }
        
    }
}


impl ProtocolHandler for NetworkManager {
    async fn accept(
        &self,
        connection: iroh::endpoint::Connection,
    ) -> Result<(), iroh::protocol::AcceptError>
    {
        let (mut greet_write, mut greet_read) = connection.accept_bi().await?;
        let client = connection.remote_id().to_string();
        println!("[syncho] Client connected from {}", client);
        
        let username;
        let result = timeout(Duration::from_secs(5), self.greet(&mut greet_read, &mut greet_write)).await;
        if let Ok(Ok(client_name)) = result {
            username = client_name;
        } else {
            connection.close(1u8.into(), b"Client did not send name");
            return Ok(());
        }

        // Send the current song and close the stream
        let curr_song = { self.play_state.lock().await.get_current_song().await };
        let message = match curr_song {
            Some(song) => NetworkMessage::CurrentState { song: Some(song) },
            None => NetworkMessage::CurrentState { song: None },
        };
        greet_write.write_all(&message.to_bytes()).await;
        greet_write.flush().await;
        greet_write.finish();



        connection.close(1u8.into(), b"end_jam");

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

