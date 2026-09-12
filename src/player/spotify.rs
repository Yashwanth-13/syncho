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

    fn is_expired_token_error(status: StatusCode, body: &Value) -> bool {
        if status != StatusCode::UNAUTHORIZED {
            return false;
        }
        body.get("error")
            .and_then(|e| e.get("message"))
            .and_then(|m| m.as_str())
            .map(|m| m.to_lowercase().contains("expired") || m.to_lowercase().contains("invalid"))
            .unwrap_or(false)
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
        let position = (body.get("progress_ms")?.as_i64()? / 1000).to_string();

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

    pub async fn play_pause(&mut self) -> Result<()> {
        // TODO
        Ok(())
    }

    pub async fn play(&mut self) -> Result<()> {
        // TODO
        Ok(())
    }
}
