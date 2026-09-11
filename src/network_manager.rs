use std::io::{BufReader, BufWriter};

use tokio::{net::TcpStream};

pub async fn handle_connection(mut stream: TcpStream) -> Option<()>{
    let (reader, writer) = stream.split();
    Some(())
}