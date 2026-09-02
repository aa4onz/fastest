// src/network/http.rs
use crate::models::{Channel, Server};

pub struct DiscordHttpClient {
    pub client: reqwest::Client,
    pub token: String,
}

impl DiscordHttpClient {
    pub fn new(client: reqwest::Client, token: String) -> Self {
        Self { client, token }
    }

    /// Fetches all guilds (servers) the user belongs to
    pub async fn fetch_guilds(&self) -> Result<Vec<Server>, reqwest::Error> {
        let url = "https://discord.com";
        
        let res = self.client.get(url)
            .header("Authorization", &self.token) 
            .header("Content-Type", "application/json")
            .send()
            .await?;
            
        res.json::<Vec<Server>>().await
    }

    /// Fetches all channels belonging to a specific server guild ID
    pub async fn fetch_channels(&self, server_id: &str) -> Result<Vec<Channel>, reqwest::Error> {
        let url = format!("https://discord.com/api/v10/channels/{}/channels", server_id);
        let res = self.client.get(&url)
            .header("Authorization", &self.token)
            .header("Content-Type", "application/json")
            .send()
            .await?;
            
        let mut channels = res.json::<Vec<Channel>>().await?;
        channels.retain(|c| !c.name.is_empty());
        Ok(channels)
    }

    /// ⚡ OPTIMIZED FOR MAX SPEED: Fires typing indicator down persistent TCP pool
    pub async fn send_typing(&self, channel_id: &str) -> Result<(), reqwest::Error> {
        let url = format!("https://discord.com/api/v10/channels/{}/typing", channel_id);
        let _ = self.client.post(&url)
            .header("Authorization", &self.token)
            .header("Content-Length", "0")
            .send()
            .await?;
        Ok(())
    }

    /// ⚡ OPTIMIZED FOR MAX SPEED: Sends message json frame cleanly
    pub async fn send_message(&self, channel_id: &str, text: &str, nonce: &str) -> Result<reqwest::Response, reqwest::Error> {
        let url = format!("https://discord.com/api/v10/channels/{}/messages", channel_id);
        let json_payload = serde_json::json!({
            "content": text,
            "nonce": nonce
        });

        self.client.post(&url)
            .header("Authorization", &self.token)
            .header("Content-Type", "application/json")
            .header("Accept", "*/*")
            .header("Origin", "https://discord.com")
            .header("X-Discord-Locale", "en-US")
            .json(&json_payload)
            .send()
            .await
    }
}
