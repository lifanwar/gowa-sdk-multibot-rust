use anyhow::{anyhow, Result};
use serde_json::Value;
use std::{
    future::Future,
    pin::Pin,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
};
use tokio::sync::Mutex as AsyncMutex;
use tracing::{error, info};

use crate::{
    events::{MessageEv, StartedEv},
    gowa_api::GoWaApi,
    message::{is_truthy_value, value_to_string, IncomingMessage, Payload},
    redis_pubsub::RedisPubSubSubscriber,
    settings::Settings,
};

type HandlerFuture = Pin<Box<dyn Future<Output = Result<()>> + Send + 'static>>;
type StartedHandler = Arc<dyn Fn(AutomationClient, StartedEv) -> HandlerFuture + Send + Sync>;
type MessageHandler = Arc<dyn Fn(AutomationClient, MessageEv) -> HandlerFuture + Send + Sync>;

#[derive(Clone)]
pub struct AutomationClient {
    inner: Arc<AutomationClientInner>,
}

struct AutomationClientInner {
    settings: Settings,
    subscriber: AsyncMutex<RedisPubSubSubscriber>,
    whatsapp: GoWaApi,
    started_handlers: Mutex<Vec<StartedHandler>>,
    message_handlers: Mutex<Vec<MessageHandler>>,
    running: AtomicBool,
    stopping: AtomicBool,
}

pub enum TargetMessage {
    Id(String),
    Response(Value),
}

impl From<String> for TargetMessage {
    fn from(value: String) -> Self {
        Self::Id(value)
    }
}

impl From<&str> for TargetMessage {
    fn from(value: &str) -> Self {
        Self::Id(value.to_string())
    }
}

impl From<Value> for TargetMessage {
    fn from(value: Value) -> Self {
        Self::Response(value)
    }
}

impl From<&Value> for TargetMessage {
    fn from(value: &Value) -> Self {
        Self::Response(value.clone())
    }
}

impl AutomationClient {
    pub fn new() -> Result<Self> {
        let settings = Settings::from_env()?;
        Self::with_settings(settings)
    }

    pub fn with_settings(settings: Settings) -> Result<Self> {
        let subscriber = RedisPubSubSubscriber::new(settings.clone())?;
        let whatsapp = GoWaApi::new(settings.clone())?;

        Ok(Self {
            inner: Arc::new(AutomationClientInner {
                settings,
                subscriber: AsyncMutex::new(subscriber),
                whatsapp,
                started_handlers: Mutex::new(Vec::new()),
                message_handlers: Mutex::new(Vec::new()),
                running: AtomicBool::new(false),
                stopping: AtomicBool::new(false),
            }),
        })
    }

    pub fn settings(&self) -> &Settings {
        &self.inner.settings
    }

    pub fn on_started<F, Fut>(&self, handler: F)
    where
        F: Fn(AutomationClient, StartedEv) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<()>> + Send + 'static,
    {
        let handler: StartedHandler = Arc::new(move |client, event| Box::pin(handler(client, event)));
        self.inner
            .started_handlers
            .lock()
            .expect("started handler mutex poisoned")
            .push(handler);
    }

    pub fn on_message<F, Fut>(&self, handler: F)
    where
        F: Fn(AutomationClient, MessageEv) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<()>> + Send + 'static,
    {
        let handler: MessageHandler = Arc::new(move |client, event| Box::pin(handler(client, event)));
        self.inner
            .message_handlers
            .lock()
            .expect("message handler mutex poisoned")
            .push(handler);
    }

    async fn emit_started(&self, event: StartedEv) -> Result<()> {
        let handlers = self
            .inner
            .started_handlers
            .lock()
            .expect("started handler mutex poisoned")
            .clone();

        for handler in handlers {
            handler(self.clone(), event.clone()).await?;
        }

        Ok(())
    }

    async fn emit_message(&self, event: MessageEv) -> Result<()> {
        let handlers = self
            .inner
            .message_handlers
            .lock()
            .expect("message handler mutex poisoned")
            .clone();

        for handler in handlers {
            handler(self.clone(), event.clone()).await?;
        }

        Ok(())
    }

    pub async fn start(&self) -> Result<()> {
        {
            let mut subscriber = self.inner.subscriber.lock().await;
            subscriber.subscribe().await?;
        }

        let started_event = StartedEv {
            bot_name: self.inner.settings.bot_name.clone(),
            device_id: self.inner.settings.device_id.clone(),
            channel_name: self.inner.settings.resolved_pubsub_channel(),
        };

        self.emit_started(started_event).await?;
        self.inner.running.store(true, Ordering::SeqCst);

        while self.inner.running.load(Ordering::SeqCst) {
            let events = {
                let mut subscriber = self.inner.subscriber.lock().await;
                subscriber.read().await?
            };

            if events.is_empty() {
                continue;
            }

            for payload in events {
                match self.build_message_event(payload.clone()) {
                    Ok(message_event) => {
                        if let Err(error) = self.emit_message(message_event).await {
                            error!(
                                error = %error,
                                channel = %self.inner.settings.resolved_pubsub_channel(),
                                payload = ?payload,
                                "handler_error"
                            );
                        }
                    }
                    Err(error) => {
                        error!(
                            error = %error,
                            channel = %self.inner.settings.resolved_pubsub_channel(),
                            payload = ?payload,
                            "build_message_event_error"
                        );
                    }
                }
            }
        }

        Ok(())
    }

    pub async fn stop(&self) -> Result<()> {
        if self.inner.stopping.swap(true, Ordering::SeqCst) {
            return Ok(());
        }

        self.inner.running.store(false, Ordering::SeqCst);

        let mut subscriber = self.inner.subscriber.lock().await;
        subscriber.close().await?;

        info!("worker_stopped");
        Ok(())
    }

    pub async fn run(&self) -> Result<()> {
        let result = self.start().await;
        let stop_result = self.stop().await;

        result?;
        stop_result?;

        Ok(())
    }

    pub fn build_message_event(&self, payload: Payload) -> Result<MessageEv> {
        let chat_id = value_to_string(payload.get("chat_id"));
        let sender = value_to_string(payload.get("sender")).or_else(|| value_to_string(payload.get("from")));
        let device_id = value_to_string(payload.get("device_id"))
            .unwrap_or_else(|| self.inner.settings.device_id.clone());

        let is_group = payload.get("is_group").is_some_and(is_truthy_value)
            || chat_id
                .as_deref()
                .is_some_and(|chat_id| chat_id.ends_with("@g.us"));

        let message = IncomingMessage {
            id: value_to_string(payload.get("message_id")).or_else(|| value_to_string(payload.get("id"))),
            text: value_to_string(payload.get("text"))
                .or_else(|| value_to_string(payload.get("body")))
                .unwrap_or_default(),
            sender,
            chat_id,
            device_id,
            is_group,
            raw: payload.clone(),
        };

        Ok(MessageEv {
            event_id: value_to_string(payload.get("event_id"))
                .or_else(|| value_to_string(payload.get("message_id")))
                .or_else(|| value_to_string(payload.get("id"))),
            device_id: message.device_id.clone(),
            message,
            raw: payload.clone(),
            event_name: value_to_string(payload.get("event")).unwrap_or_else(|| "message".to_string()),
            channel_name: self.inner.settings.resolved_pubsub_channel(),
        })
    }

    pub async fn send_message(&self, text: &str, message: &IncomingMessage) -> Result<Value> {
        let target = message
            .contact_id()
            .ok_or_else(|| anyhow!("Cannot send message because message contact_id is empty"))?;

        self.inner
            .whatsapp
            .send_message(&target, text, None, None, None, None, None)
            .await
    }

    pub async fn reply_message(&self, text: &str, message: &IncomingMessage) -> Result<Value> {
        self.inner
            .whatsapp
            .send_message(
                &message
                    .contact_id()
                    .ok_or_else(|| anyhow!("Cannot reply message because message contact_id is empty"))?,
                text,
                message.id.as_deref(),
                None,
                None,
                None,
                None,
            )
            .await
    }

    pub async fn update_message<T>(
        &self,
        text: &str,
        target_message: T,
        message: &IncomingMessage,
    ) -> Result<Value>
    where
        T: Into<TargetMessage>,
    {
        let message_id = match target_message.into() {
            TargetMessage::Id(message_id) => Some(message_id),
            TargetMessage::Response(value) => value
                .get("message_id")
                .and_then(Value::as_str)
                .map(ToString::to_string),
        }
        .ok_or_else(|| anyhow!("Cannot update message because message_id is empty"))?;

        let target = message
            .contact_id()
            .ok_or_else(|| anyhow!("Cannot update message because message contact_id is empty"))?;

        self.inner
            .whatsapp
            .update_message(&message_id, &target, text)
            .await
    }
}
