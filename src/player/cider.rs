use cider_api::CiderClient;

use dict::{Dict, DictIface};
use serde::Deserialize;
use strsim::jaro_winkler;


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

pub struct CiderControl {
    pub cider: CiderClient
}

impl CiderControl {

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

        song_sim * 0.4 + artist_sim * 0.45 + album_sim * 0.15
    }

    fn find_best_match(
        json_str: &str,
        target_song: &str,
        target_artist: &str,
        target_album: &str,
    ) -> Option<String> {
        let parsed: SearchResponse = serde_json::from_str(json_str).ok()?;

        parsed
            .data
            .results
            .songs
            .data
            .iter()
            .map(|entry| (entry, CiderControl::score_candidate(&entry.attributes, target_song, target_artist, target_album)))
            .max_by(|(_, score_a), (_, score_b)| score_a.partial_cmp(score_b).unwrap())
            .map(|(entry, _)| entry.id.clone())
    }

    pub fn new(token: &String) -> CiderControl {
        CiderControl { cider: CiderClient::new().with_token(token) }
    }

    pub async fn play(&self, song_name: &String, artist_name: &String, album_name: &String) {
        let song_id = self.get_id(song_name, artist_name, album_name).await;
        
        if let Some(id) = song_id {
            println!("{:?}", id);
            self.cider.play_item("songs", &id).await;
        }
    }

    async fn get_id(&self, song_name: &String, artist_name: &String, album_name: &String) -> Option<String>{
        let path = format!("/v1/catalog/in/search?types=songs&term={}", song_name);

        let response = self.cider.amapi_run_v3(&path).await.unwrap().to_string();
        let best_match = CiderControl::find_best_match(&response, song_name, artist_name, album_name);

        best_match
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
}