use std::sync::Arc;
use crate::{network_manager::helpers::{process_event, send}, player::types::{PlayState, Song}};
use serde::{Deserialize, Serialize};
use tokio::sync::{Mutex, broadcast};
use iroh::{protocol::ProtocolHandler};


#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum NetworkMessage {
    Hello,
    Welcome,
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
    pub code: String,
    pub bc: HostBroadcaster,
    pub play_state: Arc<Mutex<PlayState>>
}


impl NetworkManager {
    async fn connect() {
        todo!();
    }
    async fn greet_and_authenticate( &self, reader: &Arc<Mutex<iroh::endpoint::RecvStream>>, writer: &Arc<Mutex<iroh::endpoint::SendStream>> ) -> bool {
        let mut rr_locked = reader.lock().await;

        let message_bytes = rr_locked.read_to_end(1000).await.unwrap();

        let message = NetworkMessage::from_bytes(&message_bytes);
        match message {
            NetworkMessage::Hello => {
                send(&writer, NetworkMessage::Welcome).await;
            },

            _ => {}
        }

        let mut is_authenticated = false;
        while !is_authenticated {
            let response_bytes = rr_locked.read_to_end(1000).await.unwrap();
            let response = NetworkMessage::from_bytes(&response_bytes);
            match response {
                NetworkMessage::Auth { code } => {
                    if code == self.code {
                        is_authenticated = true;
                        send(writer, NetworkMessage::AuthOk).await;
                    } else {
                        send(writer, NetworkMessage::AuthFail).await;
                    }
                },

                NetworkMessage::Bye => {
                    break;
                }

                _ => {
                    send(writer, NetworkMessage::AuthFail).await;
                }
            }
        };

        is_authenticated

    }
}

// !!! need to think on the stream design for connections
impl ProtocolHandler for NetworkManager {
    async fn accept(
        &self,
        connection: iroh::endpoint::Connection,
    ) -> Result<(), iroh::protocol::AcceptError>
    {
        let (write_half, read_half) = connection.accept_bi().await?;
        let client = connection.remote_id().to_string();
        println!("[syncho] Client connected from {}", client);

        let auth_reader: Arc<Mutex<iroh::endpoint::RecvStream>> = Arc::new(Mutex::new(read_half));
        let auth_writer: Arc<Mutex<iroh::endpoint::SendStream>> = Arc::new(Mutex::new(write_half));
        
        let is_successful = self.greet_and_authenticate(&auth_reader, &auth_writer).await;

        if !is_successful {
            connection.close(1u8.into(), b"client_disconnect");
            return Ok(());
        } else {
            // Send the current song and close the stream
            let curr_song = { self.play_state.lock().await.get_current_song().await };
            let message = match curr_song {
                Some(song) => NetworkMessage::CurrentState { song: Some(song) },
                None => NetworkMessage::CurrentState { song: None },
            };
            let _ = send(&auth_writer, message).await;
            auth_writer.lock().await.finish();
            auth_reader.lock().await.stop(1u8.into());
        }

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
