use crate::message::{IncomingMessage, Payload};

#[derive(Debug, Clone)]
pub struct StartedEv {
    pub bot_name: String,
    pub device_id: String,
    pub channel_name: String,
}

#[derive(Debug, Clone)]
pub struct MessageEv {
    pub device_id: String,
    pub message: IncomingMessage,
    pub raw: Payload,
    pub event_name: String,
    pub channel_name: String,
    pub event_id: Option<String>,
}
