use cider_api::{CiderClient, CiderError};
use std::{thread::sleep, time::Duration};
use url::form_urlencoded;

use super::types::*;
use serde::Deserialize;
use strsim::normalized_levenshtein;
use unicode_normalization::UnicodeNormalization;

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
        CiderControl {
            cider: CiderClient::new().with_token(token),
        }
    }

    fn is_combining_mark(c: char) -> bool {
        // Combining Diacritical Marks block: U+0300–U+036F
        matches!(c, '\u{0300}'..='\u{036F}')
    }

    // Lowercases and strips the names
    fn normalize(s: &str) -> String {
        let deaccented: String = s
            .nfd()
            .filter(|c| !CiderControl::is_combining_mark(*c))
            .collect();

        let lower = deaccented
            .to_lowercase()
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

        match paren_stripped.find(" - ") {
            Some(idx) => paren_stripped[..idx].trim().to_string(),
            None => paren_stripped,
        }
    }

    /// Scores a song against a target song using `Levenshtein` 
    fn score_candidate(
        candidate: &SongAttributes,
        target_song: &str,
        target_artist: &str,
        target_album: &str,
    ) -> f64 {
        let min_song_changes = normalized_levenshtein(
            &CiderControl::normalize(&candidate.name),
            &CiderControl::normalize(target_song),
        );
        let min_artist_changes = normalized_levenshtein(
            &CiderControl::normalize(&candidate.artist_name),
            &CiderControl::normalize(target_artist),
        );
        let min_album_changes = normalized_levenshtein(
            &CiderControl::normalize(&candidate.album_name),
            &CiderControl::normalize(target_album),
        );

        min_song_changes * 0.4 + min_artist_changes * 0.45 + min_album_changes * 0.15
    }

    /// Gets a song that is the closest to the target song
    fn find_best_match(json_str: &str, song: &Song) -> Option<String> {
        let parsed: SearchResponse = serde_json::from_str(json_str).ok()?;

        parsed
            .data
            .results
            .songs
            .data
            .iter()
            .map(|entry| {
                (
                    entry,
                    CiderControl::score_candidate(
                        &entry.attributes,
                        &song.song_name,
                        &song.artist_name,
                        &song.album_name,
                    ),
                )
            })
            .max_by(|(_, score_a), (_, score_b)| score_a.partial_cmp(score_b).unwrap())
            .map(|(entry, _)| entry.id.clone())
    }

    /// Fetches the Apple Music Song ID of the song closest to the given one
    async fn get_id(&self, song: &Song) -> Option<String> {
        let encoded_song: String =
            form_urlencoded::byte_serialize(song.song_name.as_bytes()).collect();
        let path = format!("/v1/catalog/in/search?types=songs&term={}", encoded_song);

        let mut back_off: f64 = 1.0;
        let mut count = 0;
        loop {
            match self.cider.amapi_run_v3(&path).await {
                Ok(val) => {
                    let best_match = CiderControl::find_best_match(&val.to_string(), song);
                    return best_match;
                }
                Err(_) => {
                    if count > 5 {
                        println!("Max hit reached. Aborting request");
                        return None;
                    }
                    println!("Error. Retrying again after: {}s", back_off);
                    sleep(Duration::from_secs_f64(back_off));
                    back_off *= 1.5;
                    count += 1;
                }
            }
        }
    }

    /// Fetches current playing song in `Cider`
    pub async fn get_current_song(&self) -> Option<Song> {
        if let Ok(track) = self.cider.now_playing().await {
            match track {
                Some(track) => Some(Song {
                    song_name: track.name.clone(),
                    artist_name: track.artist_name.clone(),
                    album_name: track.album_name.clone(),
                    position: track.current_position_ms(),
                }),

                _ => None,
            }
        } else {
            None
        }
    }

    /// Wrapper to seek song position
    pub async fn seek(&self, new_positon: u64) {
        let _ = self.cider.seek_ms(new_positon).await;
    }

    /// Plays a specific song
    pub async fn play(&self, song: &Song) -> Result<(), CiderError> {
        let song_id = self.get_id(song).await.unwrap();
        let _ = self.cider.play_item("songs", &song_id).await;
        self.cider.seek_ms(song.position).await
    }

    /// Wrapper to resume music
    pub async fn resume(&self) -> Result<(), CiderError> {
        self.cider.play().await
    }

    /// Wrapper to pause music
    pub async fn pause(&self) -> Result<(), CiderError> {
        self.cider.pause().await
    }

    /// Adds a song to the queue
    pub async fn play_next(&self, song: &Song) -> Result<(), CiderError> {
        let song_id = self.get_id(song).await.unwrap();
        self.cider.play_next("songs", &song_id).await
    }

    /// Fetches the next 10 songs in the queue (current + next 9)
    pub async fn get_queue(&self) -> Vec<String> {
        let queue = self.cider.get_queue().await.unwrap();
        let now_playing_result = self.cider.now_playing().await;

        match now_playing_result {
            Ok(Some(song)) => {

                // This is the song we need to find in the queue and build a list from
                let target = Song {
                    song_name: song.name,
                    artist_name: song.artist_name,
                    album_name: song.album_name,
                    position: 0
                };

                let mut songs: Vec<String> = Vec::new(); // List of upcoming songs
                let mut got_current = false;
                let mut queue_size = 0;
                for i in queue {
                    let attr = i.attributes.unwrap();
                    let attr_clone = attr.clone();
                    let queue_item = Song{
                        song_name: attr_clone.name,
                        artist_name:attr_clone.artist_name,
                        album_name: attr_clone.album_name,
                        position: 0
                    };

                    // Check if we already got the current song from past queue items
                    if !got_current {
                        got_current = CiderControl::is_same(&target, &queue_item); // Check if the current queue item is the current song
                    }

                    // Check again since the current queue item might have matched
                    // Also limit the queue size to be 10 
                    if got_current && queue_size < 10 {
                        let formatted = format!(
                            "{} by {} from {}",
                            attr.name, attr.artist_name, attr.album_name
                        );
                        
                        songs.push(formatted);
                        queue_size += 1;
                    }
                }

                songs
            }

            _ => {vec![]}
        }
    }

    /// Returns if two songs are the same
    fn is_same(a: &Song, b: &Song) -> bool {
        (a.song_name.eq(&b.song_name)) & 
        (a.artist_name.eq(&b.artist_name)) & 
        (a.album_name.eq(&b.album_name))
    }
}
