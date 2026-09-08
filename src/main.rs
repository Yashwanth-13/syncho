mod cider;
mod config_manager;
mod helpers;

use clap::{Parser};
use crate::{cider::CiderControl, helpers::{generate_numeric_code, get_input}};

#[derive(Parser, Debug)]
#[command(author, version, about)]
struct Args {

    #[arg(long = "host")]
    host: bool,

    #[arg(long = "join")]
    join: bool,
}

enum Player {
    Cider(CiderControl),
    Spotify
}

#[tokio::main]
async fn main() {
    let config = config_manager::get_config();

    let mut player;
    if config.player == "Cider" {
        player = Player::Cider(CiderControl::new(&config.token));
    } else {
        player = Player::Spotify;
    }

    let args = Args::parse();
    if args.host {
        let code = generate_numeric_code();
        println!("{}", code);
    } else if args.join {
        let code = get_input(&"Session Code".to_string());
    }
}
