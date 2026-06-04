use anyhow::{Context, Result};
use std::{env, process};

#[derive(Debug, Clone)]
pub struct Settings {
    pub bot_name: String,
    pub device_id: String,
    pub redis_url: String,
    pub pubsub_channel_name: Option<String>,
    pub pubsub_channel_prefix: String,
    pub pubsub_poll_timeout_seconds: f64,
    pub redis_reconnect_sleep_seconds: f64,
    pub gowa_base_url: String,
    pub gowa_device_id: Option<String>,
    pub gowa_send_message_path: String,
    pub gowa_basic_auth_username: Option<String>,
    pub gowa_basic_auth_password: Option<String>,
}

impl Settings {
    pub fn from_env() -> Result<Self> {
        dotenvy::dotenv().ok();

        Ok(Self {
            bot_name: env_or("BOT_NAME", "automation-bot"),
            device_id: env::var("DEVICE_ID").context("DEVICE_ID is required")?,
            redis_url: env_or("REDIS_URL", "redis://localhost:6379/0"),
            pubsub_channel_name: env_optional("PUBSUB_CHANNEL_NAME"),
            pubsub_channel_prefix: env_or("PUBSUB_CHANNEL_PREFIX", "wa:incoming"),
            pubsub_poll_timeout_seconds: parse_env_or("PUBSUB_POLL_TIMEOUT_SECONDS", 1.0),
            redis_reconnect_sleep_seconds: parse_env_or("REDIS_RECONNECT_SLEEP_SECONDS", 2.0),
            gowa_base_url: env_or("GOWA_BASE_URL", "http://localhost:3000"),
            gowa_device_id: env_optional("GOWA_DEVICE_ID"),
            gowa_send_message_path: env_or("GOWA_SEND_MESSAGE_PATH", "/send/message"),
            gowa_basic_auth_username: env_optional("GOWA_BASIC_AUTH_USERNAME"),
            gowa_basic_auth_password: env_optional("GOWA_BASIC_AUTH_PASSWORD"),
        })
    }

    pub fn resolved_pubsub_channel(&self) -> String {
        match &self.pubsub_channel_name {
            Some(channel) if !channel.trim().is_empty() => channel.clone(),
            _ => format!("{}:{}", self.pubsub_channel_prefix, self.device_id),
        }
    }

    pub fn resolved_consumer_name(&self) -> String {
        let hostname = env::var("HOSTNAME").unwrap_or_else(|_| "localhost".to_string());
        format!("{}-{}-{}", self.bot_name, hostname, process::id())
    }

    pub fn resolved_gowa_device_id(&self) -> String {
        self.gowa_device_id
            .clone()
            .unwrap_or_else(|| self.device_id.clone())
    }
}

fn env_or(key: &str, default: &str) -> String {
    env::var(key).unwrap_or_else(|_| default.to_string())
}

fn env_optional(key: &str) -> Option<String> {
    env::var(key).ok().filter(|value| !value.trim().is_empty())
}

fn parse_env_or<T>(key: &str, default: T) -> T
where
    T: std::str::FromStr,
{
    env::var(key)
        .ok()
        .and_then(|value| value.parse::<T>().ok())
        .unwrap_or(default)
}
