use dialoguer::{Select};
use ping;
use std::{env, fs, io::{self, BufWriter}};
use std::{self, path::Path, fs::File, net::Ipv4Addr};


use crate::helpers::get_input;
use crate::player::types::Config;

impl Config {
    fn is_reachable(peer_ip: &String) -> bool {
        let ip = peer_ip.parse::<Ipv4Addr>();

        match ip {
            Ok(val) => {
                match ping::new(std::net::IpAddr::V4(val))
                .send() {
                    Ok(_) => true,
                    Err(_) => false
                }
            },
            Err(e) => false
        }
    }

    fn get_ip() -> String {
        loop {
            let peer_ip = get_input(&"Friend's IP address".to_string(), false);

            if Config::is_reachable(&peer_ip) {
                return peer_ip
            } else {
                println!("Enter a valid IP address\n")
            }
        }
    }

    fn create_config(path: &Path) -> Config {
        println!("\nNo previous config found. Creating new config\n");
        
        let valid_players = vec!["Cider", "Spotify"];
        let api_key: String;

        let selection = Select::new()
            .with_prompt("What is your music player?")
            .items(&valid_players)
            .interact()
            .unwrap();

        api_key = get_input(&format!("Enter your API key for {}" , valid_players[selection]), false);


        let file = File::create_new(path).unwrap();

        let cnfg = Config {player: valid_players[selection].to_string(), token: api_key};

        serde_json::to_writer(BufWriter::new(file), &cnfg).unwrap();

        cnfg

    }

    pub fn get_config() -> Config {
        let file_path = format!("{}/.syncho", env::home_dir().unwrap().to_str().unwrap());
        let path = Path::new(&file_path);
        
        if path.exists() {
            let config_data = fs::read_to_string(&file_path).unwrap();
            let config: Config = serde_json::from_str(&config_data).unwrap();
            config
        } else {
            Config::create_config(&path)
        }
    }
}
