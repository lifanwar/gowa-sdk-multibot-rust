use anyhow::{anyhow, Result};
use reqwest::{header, Client};
use serde_json::{json, Map, Value};
use urlencoding::encode;

use crate::settings::Settings;

#[derive(Debug, Clone)]
pub struct GoWaApi {
    settings: Settings,
    client: Client,
    base_url: String,
}

impl GoWaApi {
    pub fn new(settings: Settings) -> Result<Self> {
        let mut headers = header::HeaderMap::new();
        headers.insert(
            "X-Device-Id",
            header::HeaderValue::from_str(&settings.resolved_gowa_device_id())?,
        );

        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .default_headers(headers)
            .build()?;

        let base_url = settings.gowa_base_url.trim_end_matches('/').to_string();

        Ok(Self {
            settings,
            client,
            base_url,
        })
    }

    async fn post(&self, path: &str, payload: &Map<String, Value>) -> Result<Value> {
        let url = format!("{}{}", self.base_url, path);
        let mut request = self.client.post(url).json(payload);

        if let (Some(username), Some(password)) = (
            &self.settings.gowa_basic_auth_username,
            &self.settings.gowa_basic_auth_password,
        ) {
            request = request.basic_auth(username, Some(password));
        }

        let response = request.send().await?.error_for_status()?;
        let bytes = response.bytes().await?;

        if bytes.is_empty() {
            return Ok(json!({ "ok": true }));
        }

        let mut data: Value = serde_json::from_slice(&bytes)?;

        if !data.is_object() {
            return Ok(json!({ "ok": true, "data": data }));
        }

        let message_id = data
            .get("results")
            .and_then(Value::as_object)
            .and_then(|results| results.get("message_id"))
            .cloned();

        if let Some(message_id) = message_id {
            if let Some(object) = data.as_object_mut() {
                object.entry("message_id".to_string()).or_insert(message_id);
            }
        }

        Ok(data)
    }

    pub async fn send_message(
        &self,
        to: &str,
        text: &str,
        reply_message_id: Option<&str>,
        is_forwarded: Option<bool>,
        duration: Option<i64>,
        mentions: Option<Vec<String>>,
        extra_payload: Option<Map<String, Value>>,
    ) -> Result<Value> {
        if to.trim().is_empty() {
            return Err(anyhow!("to is required"));
        }

        if text.trim().is_empty() {
            return Err(anyhow!("text is required"));
        }

        let mut payload = Map::new();
        payload.insert("phone".to_string(), json!(to));
        payload.insert("message".to_string(), json!(text));

        if let Some(reply_message_id) = reply_message_id {
            payload.insert("reply_message_id".to_string(), json!(reply_message_id));
        }

        if let Some(is_forwarded) = is_forwarded {
            payload.insert("is_forwarded".to_string(), json!(is_forwarded));
        }

        if let Some(duration) = duration {
            payload.insert("duration".to_string(), json!(duration));
        }

        if let Some(mentions) = mentions {
            payload.insert("mentions".to_string(), json!(mentions));
        }

        if let Some(extra_payload) = extra_payload {
            payload.extend(extra_payload);
        }

        self.post(&self.settings.gowa_send_message_path, &payload).await
    }

    pub async fn update_message(&self, message_id: &str, to: &str, text: &str) -> Result<Value> {
        if message_id.trim().is_empty() {
            return Err(anyhow!("message_id is required"));
        }

        if to.trim().is_empty() {
            return Err(anyhow!("to is required"));
        }

        if text.trim().is_empty() {
            return Err(anyhow!("text is required"));
        }

        let safe_message_id = encode(message_id);
        let path = format!("/message/{}/update", safe_message_id);

        let mut payload = Map::new();
        payload.insert("phone".to_string(), json!(to));
        payload.insert("message".to_string(), json!(text));

        self.post(&path, &payload).await
    }
}
