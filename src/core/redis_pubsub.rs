use anyhow::Result;
use futures_util::StreamExt;
use redis::aio::PubSub;
use serde_json::{Map, Value};
use tokio::time::{sleep, timeout, Duration, Instant};
use tracing::warn;

use super::settings::Settings;

pub struct RedisPubSubSubscriber {
    settings: Settings,
    client: redis::Client,
    pubsub: Option<PubSub>,

    reconnect_attempts: u32,
    next_reconnect_at: Option<Instant>,
}

impl RedisPubSubSubscriber {
    pub fn new(settings: Settings) -> Result<Self> {
        let client = redis::Client::open(settings.redis_url.as_str())?;

        Ok(Self {
            settings,
            client,
            pubsub: None,
            reconnect_attempts: 0,
            next_reconnect_at: None,
        })
    }

    fn reconnect_delay(&self) -> Duration {
        let base_seconds = self.settings.redis_reconnect_sleep_seconds.max(1.0);

        let multiplier = 2_f64.powi(self.reconnect_attempts.min(6) as i32);
        let delay_seconds = (base_seconds * multiplier).min(60.0);

        Duration::from_secs_f64(delay_seconds)
    }

    fn reset_reconnect_state(&mut self) {
        self.reconnect_attempts = 0;
        self.next_reconnect_at = None;
    }

    fn schedule_reconnect(&mut self, reason: &'static str) {
        self.pubsub = None;

        let delay = self.reconnect_delay();
        self.next_reconnect_at = Some(Instant::now() + delay);
        self.reconnect_attempts = self.reconnect_attempts.saturating_add(1);

        warn!(
            reason = reason,
            delay_seconds = delay.as_secs_f64(),
            reconnect_attempts = self.reconnect_attempts,
            "redis_reconnect_scheduled"
        );
    }

    async fn wait_backoff_if_needed(&self) -> bool {
        let Some(next_reconnect_at) = self.next_reconnect_at else {
            return false;
        };

        let now = Instant::now();

        if now >= next_reconnect_at {
            return false;
        }

        let remaining = next_reconnect_at - now;

        let max_sleep = Duration::from_secs_f64(
            self.settings
                .pubsub_poll_timeout_seconds
                .max(0.2)
                .min(5.0),
        );

        sleep(remaining.min(max_sleep)).await;
        true
    }

    pub async fn subscribe(&mut self) -> Result<bool> {
        if self.pubsub.is_some() {
            return Ok(true);
        }

        if self.wait_backoff_if_needed().await {
            return Ok(false);
        }

        self.client = redis::Client::open(self.settings.redis_url.as_str())?;

        let channel = self.settings.resolved_pubsub_channel();

        let connect_timeout = Duration::from_secs(5);

        let result = timeout(connect_timeout, async {
            let mut pubsub = self.client.get_async_pubsub().await?;
            pubsub.subscribe(channel).await?;
            Ok::<PubSub, redis::RedisError>(pubsub)
        })
        .await;

        match result {
            Ok(Ok(pubsub)) => {
                self.pubsub = Some(pubsub);
                self.reset_reconnect_state();
                Ok(true)
            }
            Ok(Err(error)) => {
                warn!(error = %error, "redis_subscribe_failed");
                self.schedule_reconnect("redis_subscribe_failed");
                Ok(false)
            }
            Err(_) => {
                warn!("redis_subscribe_timeout");
                self.schedule_reconnect("redis_subscribe_timeout");
                Ok(false)
            }
        }
    }

    pub async fn close(&mut self) -> Result<()> {
        if let Some(pubsub) = self.pubsub.as_mut() {
            let _ = pubsub
                .unsubscribe(self.settings.resolved_pubsub_channel())
                .await;
        }

        self.pubsub = None;
        self.reset_reconnect_state();

        Ok(())
    }

    pub async fn read(&mut self) -> Result<Vec<Map<String, Value>>> {
        let connected = self.subscribe().await?;

        if !connected {
            return Ok(Vec::new());
        }

        let timeout_duration = Duration::from_secs_f64(
            self.settings.pubsub_poll_timeout_seconds.max(0.2),
        );

        let pubsub = match self.pubsub.as_mut() {
            Some(pubsub) => pubsub,
            None => return Ok(Vec::new()),
        };

        let message = match timeout(timeout_duration, async {
            let mut stream = pubsub.on_message();
            stream.next().await
        })
        .await
        {
            Ok(Some(message)) => message,
            Ok(None) => {
                self.schedule_reconnect("redis_pubsub_stream_closed");
                return Ok(Vec::new());
            }
            Err(_) => return Ok(Vec::new()),
        };

        let payload_raw = match message.get_payload::<String>() {
            Ok(value) => value,
            Err(error) => {
                warn!(error = %error, "redis_payload_decode_error");
                return Ok(Vec::new());
            }
        };

        let payload = match serde_json::from_str::<Value>(&payload_raw) {
            Ok(Value::Object(map)) => map,
            Ok(value) => {
                let mut map = Map::new();
                map.insert("event".to_string(), Value::String("raw".to_string()));
                map.insert("raw_payload".to_string(), value);
                map
            }
            Err(_) => {
                let mut map = Map::new();
                map.insert("event".to_string(), Value::String("raw".to_string()));
                map.insert("raw_payload".to_string(), Value::String(payload_raw));
                map
            }
        };

        Ok(vec![payload])
    }
}