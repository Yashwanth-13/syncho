use std::{hint::select_unpredictable, thread::sleep, time::Duration};

use cider_api::{CiderClient, CiderError};

use dict::{Dict, DictIface};
use serde::Deserialize;
use strsim::jaro_winkler;
use super::types::*;

#[derive(Debug, Deserialize)]
struct SearchResponse {
    data: SearchData,
}

#[derive(Debug, Deserialize)]
struct SearchData {
    results: SearchResults,
}

#[derive(Debug, Deserialize)]
struct SearchResults {
    songs: SongsBlock,
}

#[derive(Debug, Deserialize)]
struct SongsBlock {
    data: Vec<SongEntry>,
}

#[derive(Debug, Deserialize)]
struct SongEntry {
    id: String,
    attributes: SongAttributes,
}

#[derive(Debug, Deserialize)]
struct SongAttributes {
    name: String,
    #[serde(rename = "artistName")]
    artist_name: String,
    #[serde(rename = "albumName")]
    album_name: String,
}

impl CiderControl {

    pub fn new(token: &String) -> CiderControl {
        CiderControl { cider: CiderClient::new().with_token(token) }
    }

    fn normalize(s: &str) -> String {
        let lower = s.to_lowercase();
        let paren_stripped = match lower.find('(') {
            Some(idx) => lower[..idx].trim().to_string(),
            None => lower,
        };
        let dash_stripped = match paren_stripped.find(" - ") {
            Some(idx) => paren_stripped[..idx].trim().to_string(),
            None => paren_stripped,
        };
        dash_stripped
    }

    fn score_candidate(
        candidate: &SongAttributes,
        target_song: &str,
        target_artist: &str,
        target_album: &str,
    ) -> f64 {
        let song_sim = jaro_winkler(&CiderControl::normalize(&candidate.name), &CiderControl::normalize(target_song));
        let artist_sim = jaro_winkler(&CiderControl::normalize(&candidate.artist_name), &CiderControl::normalize(target_artist));
        let album_sim = jaro_winkler(&CiderControl::normalize(&candidate.album_name), &CiderControl::normalize(target_album));

        let score = song_sim * 0.4 + artist_sim * 0.45 + album_sim * 0.15;
        println!("Score: {}; Song: {}; Artist: {}; Album: {}", score, &candidate.name, &candidate.artist_name, &candidate.album_name);
        println!("Score: {}; Song: {}; Artist: {}; Album: {}\n", score, song_sim, artist_sim, album_sim);
        score

    }

    fn find_best_match(
        json_str: &str, 
        song: &Song
    ) -> Option<String> {
        let parsed: SearchResponse = serde_json::from_str(json_str).ok()?;

        parsed
            .data
            .results
            .songs
            .data
            .iter()
            .map(|entry| (entry, CiderControl::score_candidate(&entry.attributes, &song.song_name, &song.artist_name, &song.album_name)))
            .max_by(|(_, score_a), (_, score_b)| score_a.partial_cmp(score_b).unwrap())
            .map(|(entry, _)| entry.id.clone())
    }

    async fn get_id(&self, song: &Song) -> Option<String>{
        let path = format!("/v1/catalog/in/search?types=songs&term={}", song.song_name);
        
        
        // let bm = CiderControl::find_best_match(&self.cider.amapi_run_v3(&path).await.unwrap().to_string(), song);
        let mut back_off: f64 = 1.0;
        let mut count = 0;
        loop {
            match self.cider.amapi_run_v3(&path).await {
                Ok(val) => {
                    let best_match = CiderControl::find_best_match(&val.to_string(), song);
                    return best_match;
                },
                Err(_) => {
                    if count > 5 {
                        println!("Max hit reached. Aborting request");
                        return None;
                    }
                    println!("Error. Retrying again after: {}s", back_off);
                    sleep(Duration::from_secs_f64(back_off));
                    back_off = back_off * 1.5;
                    count = count + 1;
                }
            }

        }
    }

    pub async fn play(&self, song: &Song) {
        let song_id = self.get_id(song).await;
        
        if let Some(id) = song_id {
            println!("{:?}", id);
            self.cider.play_item("songs", &id).await;
        }
    }

    pub async fn play_later(&self, song: &Song) {
        let song_id = self.get_id(song).await;
        
        if let Some(id) = song_id {
            println!("{:?}", id);
            self.cider.play_later("songs", &id).await;
        }
    }
    pub async fn get_current_song(&self) -> Option<Dict::<String>> {
        if let Some(track)  = self.cider.now_playing().await.unwrap() {
            let mut song_attr = Dict::<String>::new();
            song_attr.add("song_name".to_string(), track.name);
            song_attr.add("artist_name".to_string(), track.artist_name);
            song_attr.add("album_name".to_string(), track.album_name);
            song_attr.add("position".to_string(), track.current_playback_time.to_string());
            Some(song_attr)
        } else {
            None
        }
    }

    pub async fn play_pause(&self) -> Result<(), CiderError> {
        self.cider.play_pause().await
    }

    pub async fn previous(&self) -> Result<(), CiderError> {
        self.cider.previous().await
    }

}