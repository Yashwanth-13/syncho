use anyhow::{anyhow, Result};
use base64::{engine::general_purpose::STANDARD as B64, Engine};
use reqwest::{Client, StatusCode};
use serde_json::Value;
use std::collections::HashMap;
use std::env;
use std::fs;

use super::types::Config;

#[derive(Debug, Clone)]
pub struct SpotifyPlayer {
    client: Client,
    refresh_token: String,
    client_id: String,
    client_secret: String,
    access_token: String,
}

use super::types::Song;

impl SpotifyPlayer {
    pub fn new(
        refresh_token: String,
        client_id: String,
        client_secret: String,
        access_token: String,
    ) -> Self {
        SpotifyPlayer {
            client: Client::new(),
            refresh_token,
            client_id,
            client_secret,
            access_token,
        }
    }

    fn config_path() -> String {
        format!("{}/.syncho", env::home_dir().unwrap().to_str().unwrap())
    }

    fn persist(&self) -> Result<()> {
        let cfg = Config::Spotify {
            refresh_token: self.refresh_token.clone(),
            client_id: self.client_id.clone(),
            client_secret: self.client_secret.clone(),
            access_token: self.access_token.clone(),
        };
        let data = serde_json::to_string_pretty(&cfg)?;
        fs::write(Self::config_path(), data)?;
        Ok(())
    }

    async fn refresh_access_token(&mut self) -> Result<()> {
        let auth = B64.encode(format!("{}:{}", self.client_id, self.client_secret));

        let params = [
            ("grant_type", "refresh_token"),
            ("refresh_token", self.refresh_token.as_str()),
        ];

        let resp = self
            .client
            .post("https://accounts.spotify.com/api/token")
            .header("Authorization", format!("Basic {}", auth))
            .form(&params)
            .send()
            .await?;

        let status = resp.status();
        let body: Value = resp.json().await?;

        if !status.is_success() {
            return Err(anyhow!("token refresh failed ({}): {}", status, body));
        }

        let access_token = body
            .get("access_token")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("no access_token in refresh response: {}", body))?
            .to_string();

        if let Some(new_refresh) = body.get("refresh_token").and_then(|v| v.as_str()) {
            self.refresh_token = new_refresh.to_string();
        }

        self.access_token = access_token;
        self.persist()?;

        Ok(())
    }

    fn is_expired_token_error(status: StatusCode, _body: &Value) -> bool {
        status == StatusCode::UNAUTHORIZED
    }

    pub async fn transfer_playback(&mut self, device_id: &str) -> Result<()> {
        let resp = self
            .client
            .put("https://api.spotify.com/v1/me/player")
            .bearer_auth(&self.access_token)
            .json(&serde_json::json!({
                "device_ids": [device_id],
                "play": true
            }))
            .send()
            .await?;

        let status = resp.status();
        let body: Value = resp.json().await.unwrap_or(Value::Null);

        if Self::is_expired_token_error(status, &body) {
            self.refresh_access_token().await?;
            return Box::pin(self.transfer_playback(device_id)).await;
        }

        // Give Spotify Connect a moment to wake the desktop client and switch session
        tokio::time::sleep(tokio::time::Duration::from_millis(1000)).await;

        Ok(())
    }

    async fn get_active_device_id(&mut self) -> Result<Option<String>> {
        let resp = self
            .client
            .get("https://api.spotify.com/v1/me/player/devices")
            .bearer_auth(&self.access_token)
            .send()
            .await?;

        let status = resp.status();
        let body: Value = resp.json().await?;

        if Self::is_expired_token_error(status, &body) {
            self.refresh_access_token().await?;
            return Box::pin(self.get_active_device_id()).await;
        }

        if !status.is_success() {
            return Ok(None);
        }

        let devices = body.get("devices").and_then(|v| v.as_array());

        if let Some(devices) = devices {
            // If there's an already active device, use it directly
            if let Some(active) = devices.iter().find(|d| {
                d.get("is_active").and_then(|v| v.as_bool()).unwrap_or(false)
            }) {
                let id = active.get("id").and_then(|v| v.as_str()).map(|s| s.to_string());
                return Ok(id);
            }

            // No device is currently active. Prefer "Computer" (desktop app), else first available
            let candidate = devices
                .iter()
                .find(|d| d.get("type").and_then(|v| v.as_str()) == Some("Computer"))
                .or_else(|| devices.first());

            if let Some(d) = candidate {
                if let Some(id) = d.get("id").and_then(|v| v.as_str()) {
                    let id = id.to_string();
                    let _ = self.transfer_playback(&id).await;
                    return Ok(Some(id));
                }
            }
        }

        Ok(None)
    }

    fn parse_song(body: &Value) -> Option<HashMap<String, String>> {
        let item = body.get("item")?;

        let song_name = item.get("name")?.as_str()?.to_string();
        let artist_name = item
            .get("artists")?
            .get(0)?
            .get("name")?
            .as_str()?
            .to_string();
        let album_name = item.get("album")?.get("name")?.as_str()?.to_string();
        // let position = (body.get("progress_ms")?.as_i64()? / 1000).to_string(); Need millis
        let position = (body.get("progress_ms")?.as_i64()?).to_string();

        let mut map = HashMap::new();
        map.insert("song_name".to_string(), song_name);
        map.insert("artist_name".to_string(), artist_name);
        map.insert("album_name".to_string(), album_name);
        map.insert("position".to_string(), position);

        Some(map)
    }

    pub async fn get_current_song(&mut self) -> Option<HashMap<String, String>> {
        let resp = self
            .client
            .get("https://api.spotify.com/v1/me/player/currently-playing")
            .bearer_auth(&self.access_token)
            .send()
            .await
            .ok()?;

        let status = resp.status();

        if status == StatusCode::NO_CONTENT {
            return None;
        }

        let body: Value = resp.json().await.ok()?;

        if Self::is_expired_token_error(status, &body) {
            self.refresh_access_token().await.ok()?;

            let resp = self
                .client
                .get("https://api.spotify.com/v1/me/player/currently-playing")
                .bearer_auth(&self.access_token)
                .send()
                .await
                .ok()?;

            let status = resp.status();

            if status == StatusCode::NO_CONTENT {
                return None;
            }

            let body: Value = resp.json().await.ok()?;

            return Self::parse_song(&body);
        }

        if !status.is_success() {
            return None;
        }

        Self::parse_song(&body)
    }

    pub async fn previous(&mut self) -> Result<()> {
        let resp = self
            .client
            .post("https://api.spotify.com/v1/me/player/previous")
            .bearer_auth(&self.access_token)
            .header(reqwest::header::CONTENT_LENGTH, "0")
            .body(Vec::new())
            .send()
            .await?;

        let status = resp.status();
        let body: Value = resp.json().await.unwrap_or(Value::Null);

        if Self::is_expired_token_error(status, &body) {
            self.refresh_access_token().await?;
            return Box::pin(self.previous()).await;
        }

        if !status.is_success() && status != StatusCode::NO_CONTENT {
            return Err(anyhow!("failed to skip to previous track ({})", status));
        }

        Ok(())
    }

    pub async fn next(&mut self) -> Result<()> {
        let resp = self
            .client
            .post("https://api.spotify.com/v1/me/player/next")
            .bearer_auth(&self.access_token)
            .header(reqwest::header::CONTENT_LENGTH, "0")
            .body(Vec::new())
            .send()
            .await?;

        let status = resp.status();
        let body: Value = resp.json().await.unwrap_or(Value::Null);

        if Self::is_expired_token_error(status, &body) {
            self.refresh_access_token().await?;
            return Box::pin(self.next()).await;
        }

        if !status.is_success() && status != StatusCode::NO_CONTENT {
            return Err(anyhow!("failed to skip to next track ({})", status));
        }

        Ok(())
    }

    pub async fn play_pause(&mut self) -> Result<()> {
        let resp = self
            .client
            .get("https://api.spotify.com/v1/me/player/currently-playing")
            .bearer_auth(&self.access_token)
            .send()
            .await?;

        let status = resp.status();

        if status == StatusCode::NO_CONTENT {
            // Nothing playing at all; nothing to toggle.
            return Ok(());
        }

        let body: Value = resp.json().await?;

        if Self::is_expired_token_error(status, &body) {
            self.refresh_access_token().await?;
            return Box::pin(self.play_pause()).await;
        }

        let is_playing = body.get("is_playing").and_then(|v| v.as_bool()).unwrap_or(false);

        let endpoint = if is_playing {
            "https://api.spotify.com/v1/me/player/pause"
        } else {
            "https://api.spotify.com/v1/me/player/play"
        };

        let _device_id = self.get_active_device_id().await?;

        let resp = self
            .client
            .put(endpoint)
            .bearer_auth(&self.access_token)
            .header(reqwest::header::CONTENT_LENGTH, "0")
            .body(Vec::new())
            .send()
            .await?;

        let status = resp.status();
        let body: Value = resp.json().await.unwrap_or(Value::Null);

        if Self::is_expired_token_error(status, &body) {
            self.refresh_access_token().await?;
            return Box::pin(self.play_pause()).await;
        }

        if !status.is_success() && status != StatusCode::NO_CONTENT {
            return Err(anyhow!("failed to toggle playback ({})", status));
        }

        Ok(())
    }
    
    async fn search_track(&mut self, track: &str, album: &str,artist: &str) -> Result<(String, String)> {
        let query = format!("track:{} album:{} artist:{}", track,album, artist);

        let resp = self
            .client
            .get("https://api.spotify.com/v1/search")
            .bearer_auth(&self.access_token)
            .query(&[("q", query.as_str()), ("type", "track"), ("limit", "1")])
            .send()
            .await?;

        let status = resp.status();
        let body: Value = resp.json().await?;

        if Self::is_expired_token_error(status, &body) {
            self.refresh_access_token().await?;
            return Box::pin(self.search_track(track,album, artist)).await;
        }

        if !status.is_success() {
            return Err(anyhow!("Spotify search failed ({}): {}", status, body));
        }

        let item = body
            .get("tracks")
            .and_then(|v| v.get("items"))
            .and_then(|v| v.get(0))
            .ok_or_else(|| anyhow!("track not found: {} by {}", track, artist))?;

        let track_id = item.get("id").and_then(|v| v.as_str()).unwrap_or("");
        let album_uri = item.get("album").and_then(|v| v.get("uri")).and_then(|v| v.as_str()).unwrap_or("");

        Ok((track_id.to_string(), album_uri.to_string()))
    }

    pub async fn play(&mut self, song: &Song) -> Result<()> {
        let (track_id, album_uri) = self.search_track(&song.song_name, &song.album_name, &song.artist_name).await?;
        let track_uri = format!("spotify:track:{}", track_id);

        let _device_id = self.get_active_device_id().await?;

        let resp = self
            .client
            .put("https://api.spotify.com/v1/me/player/play")
            .bearer_auth(&self.access_token)
            .json(&serde_json::json!({
                "context_uri": album_uri,
                "offset": { "uri": track_uri }
            }))
            .send()
            .await?;

        let status = resp.status();
        let body: Value = resp.json().await.unwrap_or(Value::Null);

        if Self::is_expired_token_error(status, &body) {
            self.refresh_access_token().await?;
            return Box::pin(self.play(song)).await;
        }

        if !status.is_success() && status != StatusCode::NO_CONTENT {
            return Err(anyhow!("failed to start playback ({}): {}", status, body));
        }

        Ok(())
    }


    pub async fn seek(&mut self, position_ms: u64) -> Result<()> {
        let resp = self
            .client
            .put("https://api.spotify.com/v1/me/player/seek")
            .bearer_auth(&self.access_token)
            .query(&[("position_ms", position_ms.to_string())])
            .header(reqwest::header::CONTENT_LENGTH, "0")
            .body(Vec::new())
            .send()
            .await?;

        let status = resp.status();
        let body: Value = resp.json().await.unwrap_or(Value::Null);

        if Self::is_expired_token_error(status, &body) {
            self.refresh_access_token().await?;
            return Box::pin(self.seek(position_ms)).await;
        }

        if !status.is_success() && status != StatusCode::NO_CONTENT {
            return Err(anyhow!("failed to seek ({}): {}", status, body));
        }

        Ok(())
    }

    pub async fn add_to_queue(&mut self, song: &Song) -> Result<()> {
        let (track_id, _album_uri) = self.search_track(&song.song_name,&song.album_name, &song.artist_name).await?;
        let uri = format!("spotify:track:{}", track_id);

        let _device_id = self.get_active_device_id().await?;

        let resp = self
            .client
            .post("https://api.spotify.com/v1/me/player/queue")
            .bearer_auth(&self.access_token)
            .query(&[("uri", uri.as_str())])
            .header(reqwest::header::CONTENT_LENGTH, "0")
            .body(Vec::new())
            .send()
            .await?;

        let status = resp.status();
        let body: Value = resp.json().await.unwrap_or(Value::Null);

        if Self::is_expired_token_error(status, &body) {
            self.refresh_access_token().await?;
            return Box::pin(self.add_to_queue(song)).await;
        }

        if !status.is_success() && status != StatusCode::NO_CONTENT {
            return Err(anyhow!("failed to queue track ({})", status));
        }

        Ok(())
    }
}
