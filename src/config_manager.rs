use dialoguer::{Select};
use std::{env, fs, io::{BufWriter}};
use std::{self, path::Path, fs::File};


use crate::helpers::*;
use crate::player::types::Config;

impl Config {

    fn create_config(path: &Path) -> Config {
        println!("\nNo previous config found. Creating new config\n");

        let valid_players = vec!["Cider", "Spotify"];

        let selection = Select::new()
            .with_prompt("What is your music player?")
            .items(&valid_players)
            .interact()
            .unwrap();

        let cnfg = match valid_players[selection] {
            "Cider" => {
                let token = get_input(&"Enter your Cider token".to_string(), false);
                Config::Cider { token }
            }

            "Spotify" => {
                let refresh_token = get_input(&"Enter your Spotify refresh token".to_string(), false);
                let client_id = get_input(&"Enter your Spotify client ID".to_string(), false);
                let client_secret = get_input(&"Enter your Spotify client secret".to_string(), false);
                let access_token = get_input(&"Enter your Spotify access token".to_string(), false);
                Config::Spotify { refresh_token, client_id, client_secret, access_token }
            }

            _ => unreachable!(),
        };

        // Write the file only after all inputs succeed
        let file = File::create(path).unwrap();
        serde_json::to_writer(BufWriter::new(file), &cnfg).unwrap();

        cnfg
    }

    pub fn get_config() -> Config {
        let file_path = format!("{}/.syncho", env::home_dir().unwrap().to_str().unwrap());
        let path = Path::new(&file_path);

        if path.exists() {
            let config_data = fs::read_to_string(&file_path).unwrap();
            match serde_json::from_str::<Config>(&config_data) {
                Ok(config) => config,
                Err(_) => {
                    // File is empty or corrupt (e.g. crashed during setup) — start fresh
                    println!("Config file is invalid or empty. Re-running setup...\n");
                    fs::remove_file(&file_path).unwrap();
                    Config::create_config(path)
                }
            }
        } else {
            Config::create_config(path)
        }
    }
}
