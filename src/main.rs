use dialoguer::{Input, Select};
use ping;
use std::{env, io::{self, ErrorKind, Write}, ops::Mul, string, time::Duration};
use regex::{Regex};
use std::{path::Path, fs::File, net::Ipv4Addr, process::{exit}};

fn is_valid_ip(peer_ip: &String) -> bool {
    let ip = peer_ip.parse::<Ipv4Addr>();

    match ip {
        Ok(val) => true,
        Err(e) => false
    }

}

fn create_config(path: &Path) -> File {
    println!("\nNo previous config found. Creating new config\n");
    
    let valid_players = vec!["Cider", "Spotify"];
    let mut peer_ip;
    let mut api_key: String;

    let selection = Select::new()
        .with_prompt("What is your music player?")
        .items(&valid_players)
        .interact()
        .unwrap();

    loop {        
        api_key = Input::new()
            .with_prompt(format!("Enter your API key for {}" , valid_players[selection]))
            .interact_text()
            .unwrap();

        loop {
            peer_ip = Input::new()
            .with_prompt("Friend's IP address")
            .interact_text()
            .unwrap();

            if is_valid_ip(&peer_ip) {
                break;
            } else {
                println!("\nEnter a valid IP address\n")
            }
        }

        match ping::new(peer_ip.parse().unwrap())
            .send() {
                Ok(_) => break,
                Err(_) => println!("Cannot reach host. Make sure its on the network")
            }
    }

    let mut file = File::create_new(path).unwrap();

    file.write(format!("{}\n", valid_players[selection]).as_bytes());
    file.write(format!("{}\n", peer_ip).as_bytes());
    file.write(format!("{}\n", api_key).as_bytes());

    file
}

fn get_config_file() -> File {
    let file_path = format!("{}/.syncho", env::home_dir().unwrap().to_str().unwrap());
    let path = Path::new(&file_path);
    
    if path.exists() {
        File::open(path).unwrap()
    } else {
        create_config(&path)
    }
}

fn main() {
    let config_file = get_config_file();
}
