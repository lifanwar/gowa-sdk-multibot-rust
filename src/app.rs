use anyhow::Result;
use tracing_subscriber::EnvFilter;

use crate::{AutomationClient, MessageEv, StartedEv};

pub async fn run_worker() -> Result<()> {
    init_tracing();

    let bot = AutomationClient::new()?;

    bot.on_started(|_client, event| async move { handle_started(event).await });
    bot.on_message(|client, event| async move { handle_message(client, event).await });

    bot.run().await
}

fn init_tracing() {
    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("automation_core=info,warn"));

    tracing_subscriber::fmt().with_env_filter(env_filter).init();
}

async fn handle_started(event: StartedEv) -> Result<()> {
    println!(
        r#"{{"status":"started","bot_name":"{}","device_id":"{}","channel":"{}"}}"#,
        event.bot_name,
        event.device_id,
        event.channel_name
    );

    Ok(())
}

async fn handle_message(client: AutomationClient, event: MessageEv) -> Result<()> {
    let message = event.message;
    let text = message.text.to_lowercase().trim().to_string();

    println!(
        r#"{{"event":"{}","device_id":"{}","message_id":{:?},"chat_id":{:?},"sender":{:?},"direction":"{}","text":"{}","has_media":{}}}"#,
        event.event_name,
        event.device_id,
        message.id,
        message.chat_id,
        message.sender,
        message.direction(),
        message.text,
        message.has_media()
    );
    if message.is_group {
        return Ok(());
    }

    if message.direction() == "outgoing" && text == "done" {
        client.reply_message("yes", &message).await?;
        return Ok(());
    }

    if message.direction() != "incoming" {
        return Ok(());
    }

    match text.as_str() {
        "ping" => {
            client.send_message("pong", &message).await?;
        }
        "test" => {
            let sent = client.reply_message("processing...", &message).await?;
            client.update_message("pong", &sent, &message).await?;
        }
        _ => {}
    }

    Ok(())
}
