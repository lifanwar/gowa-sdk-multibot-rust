# Automation Core Worker Rust

Project ini memakai layout Cargo standar. `src/main.rs` hanya menjadi entry point, sedangkan logic worker berada di `src/app.rs` dan core library berada di `src/core/`.

```text
automation_core_worker/
├── Cargo.toml
└── src/
    ├── main.rs
    ├── lib.rs
    ├── app.rs
    └── core/
        ├── mod.rs
        ├── client.rs
        ├── events.rs
        ├── gowa_api.rs
        ├── message.rs
        ├── redis_pubsub.rs
        └── settings.rs
```

## Cara menjalankan

```bash
cp .env.example .env
cargo run --bin worker
```

Minimal environment yang perlu diisi:

```env
DEVICE_ID=your-device-id
REDIS_URL=redis://localhost:6379/0
GOWA_BASE_URL=http://localhost:3000
```

## Catatan dependency

Dependency dibuat minimal sesuai penggunaan kode saat ini. `serde` dihapus karena belum dipakai langsung. `tokio` tidak memakai fitur `full`, tetapi hanya `macros`, `rt-multi-thread`, `sync`, dan `time`.
