
use crate::player::types::Song;
use nowhear::{MediaEvent, MediaSource, MediaSourceBuilder, PlaybackState};


// Gets current song without involving the players directly - Assumes only Cider or Spotify is playing. 
pub async fn get_playback_status() -> Option<Song> {
    let src = MediaSourceBuilder::new().build().await.unwrap();
    let players = src.list_players().await.unwrap();

    
    // if let Some(player_name) = players.first() {
    //     let player_info = src.get_player(player_name).await.unwrap();
    //     if let Some(track) = player_info.current_track {
    //         return Some(Song { song_name: track.title.clone(), artist_name: track.artist.concat(), album_name: track.album.unwrap(), position:  player_info.position.unwrap().as_millis().try_into().unwrap() });
    //     }
    // }

    None
}

pub fn is_same_player(target_player: &String, current_player: &String) -> bool {
    target_player == current_player
}