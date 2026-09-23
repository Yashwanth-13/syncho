
use nowhear::{MediaSource, MediaSourceBuilder, MediaSourceError, Track};

use crate::player::types::Song;

// Gets current track from the specified player
pub async fn get_playback_status(player: String) -> Result<Option<Track>, MediaSourceError> {
    let src = MediaSourceBuilder::new().build().await.unwrap();

    let player_obj = src.get_player(&player).await?;

    Ok(player_obj.current_track)
}

pub fn is_same_player(target_player: &String, current_player: &String) -> bool {
    target_player == current_player
}

pub fn make_song(track: Track) -> Song {
    let album = track.album;
    let album_name = match album {
        Some(name) => {name}
        None => {"".to_string()}
    };

    Song { song_name: track.title.clone(), artist_name: track.artist.concat().clone(), album_name: album_name, position: 0 }
}