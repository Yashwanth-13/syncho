use std::{thread::sleep, time::Duration};

use cider_api::{CiderClient, CiderError};

use dict::{Dict, DictIface};
use serde::Deserialize;
use strsim::{normalized_levenshtein};
use unicode_normalization::UnicodeNormalization;
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

    fn is_combining_mark(c: char) -> bool {
        // Combining Diacritical Marks block: U+0300–U+036F
        matches!(c, '\u{0300}'..='\u{036F}')
    }
    fn normalize(s: &str) -> String {
        let deaccented: String = s
        .nfd()
        .filter(|c| !CiderControl::is_combining_mark(*c))
        .collect();

        // 2. Lowercase
        let lower = deaccented.to_lowercase()
            .chars()
            .filter(|c| c.is_alphanumeric() || c.is_whitespace())
            .collect::<String>()
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");

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
        let min_song_changes = normalized_levenshtein(&CiderControl::normalize(&candidate.name), &CiderControl::normalize(target_song));
        let min_artist_changes = normalized_levenshtein(&CiderControl::normalize(&candidate.artist_name), &CiderControl::normalize(target_artist));
        let min_album_changes = normalized_levenshtein(&CiderControl::normalize(&candidate.album_name), &CiderControl::normalize(target_album));

        let score = min_song_changes * 0.4 + min_artist_changes * 0.45 + min_album_changes * 0.15;
        println!("Score: {}; Song: {}; Artist: {}; Album: {}", score, &candidate.name, &candidate.artist_name, &candidate.album_name);
        println!("Score: {}; Song: {}; Artist: {}; Album: {}\n", score, min_song_changes, min_artist_changes, min_album_changes);
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

    pub async fn play(&self, song: &Song) -> Result<(), CiderError> {
        let song_id = self.get_id(song).await.unwrap();
        println!("{:?}", song_id);
        self.cider.play_item("songs", &song_id).await
    }

    pub async fn play_later(&self, song: &Song) ->  Result<(), CiderError> {
        let song_id = self.get_id(song).await.unwrap();
        println!("{:?}", song_id);
        self.cider.play_later("songs", &song_id).await
    }

    pub async fn play_pause(&self) -> Result<(), CiderError> {
        self.cider.play_pause().await
    }

    pub async fn previous(&self) -> Result<(), CiderError> {
        self.cider.previous().await
    }

    pub async fn next(&self) -> Result<(), CiderError> {
        self.cider.next().await
    }

    pub async fn play_next(&self, song: &Song) -> Result<(), CiderError> {
        let song_id = self.get_id(song).await.unwrap();
        println!("{:?}", song_id);
        self.cider.play_next("songs", &song_id).await
    }

}