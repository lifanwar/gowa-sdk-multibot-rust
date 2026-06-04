# Automation Core Worker Rust

Ini versi Rust dengan struktur yang mirip project Python awal:

```text
automation_core_root_layout/
├── Cargo.toml
├── .env.example
├── main.rs
└── automation_core/
    ├── mod.rs
    ├── client.rs
    ├── events.rs
    ├── gowa_api.rs
    ├── message.rs
    ├── redis_pubsub.rs
    └── settings.rs
```

Catatan: ini bukan layout default Cargo. Karena `main.rs` dan folder `automation_core/` ada di root project, `Cargo.toml` memakai konfigurasi manual:

```toml
[lib]
name = "automation_core"
path = "automation_core/mod.rs"

[[bin]]
name = "worker"
path = "main.rs"
```

## Cara menjalankan

```bash
cp .env.example .env
# isi DEVICE_ID sesuai device GOWA kamu
cargo run --bin worker
```

## Cara pakai di main.rs

```rust
use automation_core::{AutomationClient, MessageEv, StartedEv};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let bot = AutomationClient::new()?;

    bot.on_started(|_client, event: StartedEv| async move {
        println!("Bot started: {}", event.bot_name);
        Ok(())
    });

    bot.on_message(|client, event: MessageEv| async move {
        let message = event.message;
        let text = message.text.to_lowercase();

        if message.direction() == "incoming" && text == "ping" {
            client.send_message("pong", &message).await?;
        }

        Ok(())
    });

    bot.run().await
}
```
