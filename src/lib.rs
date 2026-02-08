pub mod date_util;
pub mod error;
pub mod llm;
pub mod metrics;
pub mod query;
pub mod search;
pub mod storage;
pub mod sync;
pub mod url;

pub use error::{Error, Result};
pub use metrics::{
    PortfolioMetrics, ProjectMetrics, TeamMetrics, UserMetrics,
};
pub use query::builder::QueryBuilder;
pub use query::period::Period;
pub use search::{SearchHit, SearchHitType, SearchOptions, SearchResults};
pub use storage::Database;
pub use sync::{NoopProgress, SyncOptions, SyncProgress, SyncReport, SyncStatus};
pub use url::{generate_asana_url, parse_asana_url, AsanaUrlInfo};

// Re-export repository types needed by the binary crate, but not the module itself
pub use storage::repository::MonitoredEntity;

use storage::repository;
use sync::syncer;

/// Main entry point for the Asana Data Warehouse.
pub struct AsanaDW {
    db: Database,
    client: asanaclient::Client,
}

impl AsanaDW {
    pub fn new(db: Database, client: asanaclient::Client) -> Self {
        Self { db, client }
    }

    /// Access the database (for direct queries in the CLI).
    pub fn db(&self) -> &Database {
        &self.db
    }

    /// Auto-detect or retrieve the workspace GID.
    /// On first use, calls the API. If one workspace, stores it. If multiple, returns error.
    pub async fn workspace_gid(&self) -> Result<String> {
        // Check config first
        let cached: Option<String> = self
            .db
            .reader()
            .call(|conn| {
                repository::get_config(conn, "workspace_gid")
            })
            .await?;

        if let Some(gid) = cached {
            return Ok(gid);
        }

        // Auto-detect from API
        let workspaces = self.client.workspaces().list().await?;
        match workspaces.len() {
            0 => Err(Error::Config(
                "no workspaces found for this Asana token".into(),
            )),
            1 => {
                let gid = workspaces[0].gid.clone();
                self.db
                    .writer()
                    .call({
                        let gid = gid.clone();
                        move |conn| {
                            repository::set_config(conn, "workspace_gid", &gid)?;
                            Ok::<(), rusqlite::Error>(())
                        }
                    })
                    .await?;
                Ok(gid)
            }
            _ => {
                let names: Vec<String> = workspaces
                    .iter()
                    .map(|w| format!("  {} ({})", w.name, w.gid))
                    .collect();
                Err(Error::Config(format!(
                    "multiple workspaces found. Run: asanadw config set workspace_gid <GID>\n{}",
                    names.join("\n")
                )))
            }
        }
    }

    // ── User identity ────────────────────────────────────────────

    /// Auto-detect and cache the current user's identity.
    /// Checks `app_config` for `user_gid` first (no API call if cached).
    /// Calls `client.users().me()` if not cached, stores `user_gid`,
    /// `user_name`, and `user_email` in `app_config`, and upserts into `dim_users`.
    pub async fn ensure_user_identity(&self) -> Result<String> {
        // Check config first
        let cached: Option<String> = self
            .db
            .reader()
            .call(|conn| repository::get_config(conn, "user_gid"))
            .await?;

        if let Some(gid) = cached {
            return Ok(gid);
        }

        // Fetch from API
        let me = self.client.users().me().await?;
        let gid = me.gid.clone();
        let name = me.name.clone();
        let email = me.email.clone();

        self.db
            .writer()
            .call({
                let gid = gid.clone();
                let name = name.clone();
                let email = email.clone();
                move |conn| {
                    repository::set_config(conn, "user_gid", &gid)?;
                    repository::set_config(conn, "user_name", &name)?;
                    if let Some(ref email) = email {
                        repository::set_config(conn, "user_email", email)?;
                    }
                    repository::upsert_user(conn, &asanaclient::User {
                        gid: gid.clone(),
                        name,
                        email,
                        photo: None,
                    })?;
                    Ok::<(), rusqlite::Error>(())
                }
            })
            .await?;

        Ok(gid)
    }

    /// Read-only accessor for the cached user GID.
    pub async fn current_user_gid(&self) -> Result<Option<String>> {
        self.db
            .reader()
            .call(|conn| repository::get_config(conn, "user_gid"))
            .await
            .map_err(|e| Error::Database(e.to_string()))
    }

    // ── Sync commands ──────────────────────────────────────────────

    pub async fn sync_project(
        &self,
        identifier: &str,
        options: &SyncOptions,
        progress: &dyn SyncProgress,
    ) -> Result<SyncReport> {
        let gid = url::resolve_gid(identifier)?;
        syncer::sync_project(&self.db, &self.client, &gid, options, progress).await
    }

    pub async fn sync_user(
        &self,
        identifier: &str,
        options: &SyncOptions,
        progress: &dyn SyncProgress,
    ) -> Result<SyncReport> {
        let workspace_gid = self.workspace_gid().await?;
        let gid = url::resolve_gid(identifier)?;
        syncer::sync_user(&self.db, &self.client, &workspace_gid, &gid, options, progress).await
    }

    pub async fn sync_team(
        &self,
        identifier: &str,
        options: &SyncOptions,
        progress: &dyn SyncProgress,
    ) -> Result<SyncReport> {
        let workspace_gid = self.workspace_gid().await?;
        let gid = url::resolve_gid(identifier)?;
        syncer::sync_team(&self.db, &self.client, &workspace_gid, &gid, options, progress).await
    }

    pub async fn sync_portfolio(
        &self,
        identifier: &str,
        options: &SyncOptions,
        progress: &dyn SyncProgress,
    ) -> Result<SyncReport> {
        let gid = url::resolve_gid(identifier)?;
        syncer::sync_portfolio(&self.db, &self.client, &gid, options, progress).await
    }

    pub async fn sync_all(
        &self,
        options: &SyncOptions,
        progress: &dyn SyncProgress,
    ) -> Result<Vec<SyncReport>> {
        // Auto-detect user identity on first sync
        if let Err(e) = self.ensure_user_identity().await {
            log::warn!("Could not auto-detect user identity: {e}");
        }

        let entities: Vec<repository::MonitoredEntity> = self
            .db
            .reader()
            .call(|conn| {
                repository::list_monitored_entities(conn)
            })
            .await?;

        let total = entities.len();
        let mut reports = Vec::new();
        for (i, entity) in entities.iter().enumerate() {
            progress.on_entity_start(&entity.entity_key, i, total);

            let result = match entity.entity_type.as_str() {
                "project" => {
                    syncer::sync_project(&self.db, &self.client, &entity.entity_gid, options, progress).await
                }
                "user" => {
                    let ws = self.workspace_gid().await?;
                    syncer::sync_user(&self.db, &self.client, &ws, &entity.entity_gid, options, progress)
                        .await
                }
                "team" => {
                    let ws = self.workspace_gid().await?;
                    syncer::sync_team(&self.db, &self.client, &ws, &entity.entity_gid, options, progress)
                        .await
                }
                "portfolio" => {
                    syncer::sync_portfolio(&self.db, &self.client, &entity.entity_gid, options, progress)
                        .await
                }
                other => {
                    log::warn!("Unknown entity type: {other}");
                    continue;
                }
            };
            match result {
                Ok(report) => {
                    progress.on_entity_complete(&report);
                    reports.push(report);
                }
                Err(e) => {
                    log::error!("Failed to sync {}: {e}", entity.entity_key);
                    let report = SyncReport {
                        entity_key: entity.entity_key.clone(),
                        status: SyncStatus::Failed,
                        items_synced: 0,
                        items_failed: 1,
                        batches_completed: 0,
                        batches_total: 0,
                        error: Some(e.to_string()),
                    };
                    progress.on_entity_complete(&report);
                    reports.push(report);
                }
            }
        }
        Ok(reports)
    }

    // ── Monitor commands ───────────────────────────────────────────

    pub async fn monitor_add(
        &self,
        entity_type: &str,
        identifier: &str,
    ) -> Result<String> {
        let gid = url::resolve_gid(identifier)?;
        let entity_key = format!("{entity_type}:{gid}");

        // Try to get a display name
        let display_name = match entity_type {
            "project" => self
                .client
                .projects()
                .get(&gid)
                .await
                .map(|p| p.name)
                .ok(),
            "portfolio" => self
                .client
                .portfolios()
                .get(&gid)
                .await
                .map(|p| p.name)
                .ok(),
            _ => None,
        };

        self.db
            .writer()
            .call({
                let entity_key = entity_key.clone();
                let entity_type = entity_type.to_string();
                let gid = gid.clone();
                move |conn| {
                    repository::add_monitored_entity(
                        conn,
                        &entity_key,
                        &entity_type,
                        &gid,
                        display_name.as_deref(),
                    )?;
                    Ok::<(), rusqlite::Error>(())
                }
            })
            .await?;

        Ok(entity_key)
    }

    pub async fn monitor_remove(&self, entity_key: &str) -> Result<bool> {
        self.db
            .writer()
            .call({
                let entity_key = entity_key.to_string();
                move |conn| repository::remove_monitored_entity(conn, &entity_key)
            })
            .await
            .map_err(|e| Error::Database(e.to_string()))
    }

    pub async fn monitor_list(&self) -> Result<Vec<repository::MonitoredEntity>> {
        self.db
            .reader()
            .call(|conn| {
                repository::list_monitored_entities(conn)
            })
            .await
            .map_err(|e| Error::Database(e.to_string()))
    }

    /// Discover the user's favorited projects and portfolios and add them
    /// as monitored entities. Returns the list of entity keys added.
    pub async fn monitor_add_favorites(&self) -> Result<Vec<String>> {
        let workspace_gid = self.workspace_gid().await?;
        let _user_gid = self.ensure_user_identity().await?;

        let favorites = self.client.users().favorites(&workspace_gid).await?;

        let mut added = Vec::new();
        for fav in &favorites {
            let entity_type = match fav.resource_type.as_str() {
                "project" | "portfolio" => fav.resource_type.as_str(),
                _ => continue,
            };
            let entity_key = format!("{entity_type}:{}", fav.gid);
            let display_name = fav.name.as_deref();

            self.db
                .writer()
                .call({
                    let entity_key = entity_key.clone();
                    let entity_type = entity_type.to_string();
                    let gid = fav.gid.clone();
                    let display_name = display_name.map(|s| s.to_string());
                    move |conn| {
                        repository::add_monitored_entity(
                            conn,
                            &entity_key,
                            &entity_type,
                            &gid,
                            display_name.as_deref(),
                        )?;
                        Ok::<(), rusqlite::Error>(())
                    }
                })
                .await?;
            added.push(entity_key);
        }
        Ok(added)
    }

    // ── Config commands ────────────────────────────────────────────

    pub async fn config_get(&self, key: &str) -> Result<Option<String>> {
        self.db
            .reader()
            .call({
                let key = key.to_string();
                move |conn| repository::get_config(conn, &key)
            })
            .await
            .map_err(|e| Error::Database(e.to_string()))
    }

    pub async fn config_set(&self, key: &str, value: &str) -> Result<()> {
        self.db
            .writer()
            .call({
                let key = key.to_string();
                let value = value.to_string();
                move |conn| repository::set_config(conn, &key, &value)
            })
            .await
            .map_err(|e| Error::Database(e.to_string()))
    }

    pub async fn config_list(&self) -> Result<Vec<(String, String)>> {
        self.db
            .reader()
            .call(|conn| repository::list_config(conn))
            .await
            .map_err(|e| Error::Database(e.to_string()))
    }
}
