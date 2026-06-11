// file: src/bin/gs-summary.rs
// description: gs-summary binary entry point
// reference: application bootstrap and orchestration

use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    git_summarize::cli::run().await
}
