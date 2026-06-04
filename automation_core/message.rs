use serde_json::{Map, Value};

pub type Payload = Map<String, Value>;

#[derive(Debug, Clone)]
pub struct IncomingMessage {
    pub id: Option<String>,
    pub text: String,
    pub sender: Option<String>,
    pub chat_id: Option<String>,
    pub device_id: String,
    pub is_group: bool,
    pub raw: Payload,
}

impl IncomingMessage {
    pub fn is_from_me(&self) -> bool {
        self.raw
            .get("is_from_me")
            .and_then(Value::as_bool)
            .unwrap_or(false)
    }

    pub fn direction(&self) -> String {
        value_to_string(self.raw.get("direction")).unwrap_or_else(|| {
            if self.is_from_me() {
                "outgoing".to_string()
            } else {
                "incoming".to_string()
            }
        })
    }

    pub fn contact_id(&self) -> Option<String> {
        value_to_string(self.raw.get("contact_id"))
            .or_else(|| self.chat_id.clone())
            .or_else(|| self.sender.clone())
    }

    pub fn replied_to_id(&self) -> Option<String> {
        value_to_string(self.raw.get("replied_to_id"))
    }

    pub fn timestamp(&self) -> Option<String> {
        value_to_string(self.raw.get("timestamp"))
    }

    pub fn media_type(&self) -> Option<String> {
        value_to_string(self.raw.get("media_type"))
    }

    pub fn media_path(&self) -> Option<String> {
        value_to_string(self.raw.get("media_path"))
    }

    pub fn image(&self) -> Option<String> {
        if self.media_type().as_deref() == Some("image") {
            if let Some(path) = self.media_path() {
                return Some(path);
            }
        }

        value_to_string(self.raw.get("image"))
    }

    pub fn has_media(&self) -> bool {
        if self.media_type().is_some() || self.media_path().is_some() {
            return true;
        }

        ["image", "video", "audio", "document", "sticker"]
            .iter()
            .any(|key| self.raw.get(*key).is_some_and(is_truthy_value))
    }

    pub fn original_webhook_payload(&self) -> Option<Payload> {
        self.raw.get("raw")?.as_object().cloned()
    }
}

pub fn value_to_string(value: Option<&Value>) -> Option<String> {
    match value? {
        Value::String(text) if !text.is_empty() => Some(text.clone()),
        Value::Number(number) => Some(number.to_string()),
        Value::Bool(value) => Some(value.to_string()),
        _ => None,
    }
}

pub fn is_truthy_value(value: &Value) -> bool {
    match value {
        Value::Bool(value) => *value,
        Value::String(value) => !value.trim().is_empty(),
        Value::Number(_) => true,
        Value::Array(value) => !value.is_empty(),
        Value::Object(value) => !value.is_empty(),
        Value::Null => false,
    }
}
