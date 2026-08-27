// src/network/http.rs
use crate::models::{Channel, Server};

pub struct DiscordHttpClient {
    client: reqwest::Client,
    token: String,
}

impl DiscordHttpClient {
    pub fn new(client: reqwest::Client, token: String) -> Self {
        Self { client, token }
    }

    /// Fetches all guilds (servers) the user belongs to
    pub async fn fetch_guilds(&self) -> Result<Vec<Server>, reqwest::Error> {
        let url = "https://discord.com";
        
        // Print raw text if decode fails to let us see the exact error response
        let res = self.client.get(url)
            .header("Authorization", &self.token) // Clean personal token format
            .header("Content-Type", "application/json")
            .send()
            .await?;
            
        res.json::<Vec<Server>>().await
    }

    /// Fetches all channels belonging to a specific server guild ID
    pub async fn fetch_channels(&self, server_id: &str) -> Result<Vec<Channel>, reqwest::Error> {
        let url = format!("https://discord.com{}/channels", server_id);
        let res = self.client.get(&url)
            .header("Authorization", &self.token)
            .header("Content-Type", "application/json")
            .send()
            .await?;
            
        let mut channels = res.json::<Vec<Channel>>().await?;
        channels.retain(|c| !c.name.is_empty());
        Ok(channels)
    }
}
