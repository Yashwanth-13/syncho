use dialoguer::Input;
use crate::player::types::Song;

/// Helper for user prompt
pub fn get_input(prompt_string: &str, allow_empty: bool) -> String {
    Input::new().allow_empty(allow_empty).with_prompt(prompt_string).interact_text().unwrap_or("".to_string())
}

/// Gives a nice formatted string from a song's name, artist name and album name
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
