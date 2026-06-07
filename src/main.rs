use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    automation_core::run_worker().await
}
