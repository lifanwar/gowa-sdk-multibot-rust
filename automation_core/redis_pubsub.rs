use anyhow::Result;
use futures_util::StreamExt;
use redis::aio::PubSub;
use serde_json::{Map, Value};
use tokio::time::{sleep, timeout, Duration};
use tracing::warn;

use crate::settings::Settings;

pub struct RedisPubSubSubscriber {
    settings: Settings,
    client: redis::Client,
    pubsub: Option<PubSub>,
}

impl RedisPubSubSubscriber {
    pub fn new(settings: Settings) -> Result<Self> {
        let client = redis::Client::open(settings.redis_url.as_str())?;

        Ok(Self {
            settings,
            client,
            pubsub: None,
        })
    }

    pub async fn subscribe(&mut self) -> Result<()> {
        if self.pubsub.is_some() {
            return Ok(());
        }

        let mut pubsub = self.client.get_async_pubsub().await?;
        pubsub.subscribe(self.settings.resolved_pubsub_channel()).await?;
        self.pubsub = Some(pubsub);

        Ok(())
    }

    async fn reconnect(&mut self) -> Result<()> {
        self.pubsub = None;

        sleep(Duration::from_secs_f64(
            self.settings.redis_reconnect_sleep_seconds,
        ))
        .await;

        self.client = redis::Client::open(self.settings.redis_url.as_str())?;
        self.subscribe().await
    }

    pub async fn close(&mut self) -> Result<()> {
        if let Some(pubsub) = self.pubsub.as_mut() {
            let _ = pubsub
                .unsubscribe(self.settings.resolved_pubsub_channel())
                .await;
        }

        self.pubsub = None;
        Ok(())
    }

    pub async fn read(&mut self) -> Result<Vec<Map<String, Value>>> {
        self.subscribe().await?;

        let timeout_duration = Duration::from_secs_f64(self.settings.pubsub_poll_timeout_seconds);
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
            Ok(None) => return Ok(Vec::new()),
            Err(_) => return Ok(Vec::new()),
        };

        let payload_raw = match message.get_payload::<String>() {
            Ok(value) => value,
            Err(error) => {
                warn!(error = %error, "redis_payload_decode_error");
                self.reconnect().await?;
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
