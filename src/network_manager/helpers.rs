use nowhear::{Track};
use crate::player::types::Song;

pub fn is_same_player(target_player: &String, current_player: &String) -> bool {
    current_player.to_lowercase().contains(target_player)
}

pub fn make_song(track: Track) -> Song {
    let album = track.album;
    let album_name = match album {
        Some(name) => {name}
        None => {"".to_string()}
    };

    Song { song_name: track.title.clone(), artist_name: track.artist.concat().clone(), album_name: album_name, position: 0 }
}