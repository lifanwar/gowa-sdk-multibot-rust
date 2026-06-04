use anyhow::Result;
use automation_core::{AutomationClient, MessageEv, StartedEv};

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "automation_core=info,warn".into()),
        )
        .init();

    let bot = AutomationClient::new()?;

    bot.on_started(|_client: AutomationClient, event: StartedEv| async move {
        println!(
            "{{\"status\":\"started\",\"bot_name\":\"{}\",\"device_id\":\"{}\",\"channel\":\"{}\"}}",
            event.bot_name, event.device_id, event.channel_name
        );
        Ok(())
    });

    bot.on_message(|client: AutomationClient, event: MessageEv| async move {
        let message = event.message;
        let text = message.text.to_lowercase().trim().to_string();

        println!(
            "{{\"event\":\"{}\",\"device_id\":\"{}\",\"message_id\":{:?},\"chat_id\":{:?},\"sender\":{:?},\"direction\":\"{}\",\"text\":\"{}\",\"has_media\":{} }}",
            event.event_name,
            event.device_id,
            message.id,
            message.chat_id,
            message.sender,
            message.direction(),
            message.text,
            message.has_media()
        );

        // Handling group
        if message.is_group {
            return Ok(());
        }

        // Handling outgoing message
        if message.direction() == "outgoing" && text == "done" {
            client.reply_message("yes", &message).await?;
        }

        // Handling incoming message
        if message.direction() == "incoming" {
            if text == "ping" {
                client.send_message("pong", &message).await?;
                return Ok(());
            }

            if text == "test" {
                let sent = client.reply_message("processing...", &message).await?;
                client.update_message("pong", &sent, &message).await?;
                return Ok(());
            }
        }

        Ok(())
    });

    bot.run().await
}
