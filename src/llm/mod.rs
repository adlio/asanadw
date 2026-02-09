pub mod agents;

use crate::error::{Error, Result};
use crate::storage::repository;
use crate::storage::Database;

/// Create a mixtape Agent configured from the database's LLM settings.
pub async fn create_agent(db: &Database) -> Result<mixtape_core::Agent> {
    let (provider, model) = db
        .reader()
        .call(|conn| {
            let provider = repository::get_config(conn, "llm_provider")?;
            let model = repository::get_config(conn, "llm_model")?;
            Ok::<(Option<String>, Option<String>), rusqlite::Error>((provider, model))
        })
        .await?;

    let provider = provider.as_deref().unwrap_or("bedrock");
    let model_name = model.as_deref().unwrap_or("claude-sonnet-4-5");

    build_agent(provider, model_name).await
}

async fn build_agent(provider: &str, model_name: &str) -> Result<mixtape_core::Agent> {
    // Each combination needs its own builder call since the model types are different.
    match (provider, model_name) {
        ("bedrock", "claude-haiku-4-5" | "haiku") => mixtape_core::Agent::builder()
            .bedrock(mixtape_core::ClaudeHaiku4_5)
            .build()
            .await
            .map_err(|e| Error::Llm(e.to_string())),
        ("bedrock", _) => {
            // Default bedrock model
            mixtape_core::Agent::builder()
                .bedrock(mixtape_core::ClaudeSonnet4_5)
                .build()
                .await
                .map_err(|e| Error::Llm(e.to_string()))
        }
        ("anthropic", "claude-haiku-4-5" | "haiku") => mixtape_core::Agent::builder()
            .anthropic_from_env(mixtape_core::ClaudeHaiku4_5)
            .build()
            .await
            .map_err(|e| Error::Llm(e.to_string())),
        ("anthropic", _) => mixtape_core::Agent::builder()
            .anthropic_from_env(mixtape_core::ClaudeSonnet4_5)
            .build()
            .await
            .map_err(|e| Error::Llm(e.to_string())),
        (other, _) => Err(Error::Config(format!("unknown llm_provider: {other}"))),
    }
}
