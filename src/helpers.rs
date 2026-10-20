use dialoguer::Input;
use rand::RngExt;

use crate::player::types::Song;

pub fn generate_numeric_code() -> String {
    rand::rng().random_range(1111..9999).to_string()
}

pub fn get_input(prompt_string: &str, allow_empty: bool) -> String {
    if allow_empty {
        Input::new()
            .allow_empty(true)
            .with_prompt(prompt_string)
            .interact_text()
            .unwrap()
    } else {
        Input::new()
            .with_prompt(prompt_string)
            .interact_text()
            .unwrap()
    }
}

pub fn format_song(song: &Song) -> String {
    let mut formatted_song = String::new();
    formatted_song.push_str(&song.song_name);

    if !song.artist_name.is_empty() {
        formatted_song.push_str(&format!(" by {}", song.artist_name).to_string());
    }

    if !song.album_name.is_empty() {
        formatted_song.push_str(&format!(" from {}", song.album_name));
    }

    formatted_song
}
