pub mod repository;
pub mod schema;

use rusqlite_migration::{Migrations, M};

use crate::error::{Error, Result};

/// Database wraps two `tokio_rusqlite::Connection` instances (writer + reader)
/// using WAL mode for concurrent access. The writer serializes writes via
/// `tokio_rusqlite`'s internal channel; the reader can proceed without blocking.
#[derive(Clone)]
pub struct Database {
    writer: tokio_rusqlite::Connection,
    reader: tokio_rusqlite::Connection,
}

impl Database {
    /// Open the database at the default path (`~/.asanadw/asanadw.db`).
    pub async fn open() -> Result<Self> {
        let dir = dirs::home_dir()
            .ok_or_else(|| Error::Config("cannot determine home directory".into()))?
            .join(".asanadw");
        std::fs::create_dir_all(&dir).map_err(|e| Error::Config(e.to_string()))?;
        Self::open_at(dir.join("asanadw.db")).await
    }

    /// Open the database at the given path.
    pub async fn open_at(path: impl AsRef<std::path::Path>) -> Result<Self> {
        let path = path.as_ref().to_path_buf();

        let writer = tokio_rusqlite::Connection::open(&path).await?;
        Self::init_writer(&writer).await?;

        let reader = tokio_rusqlite::Connection::open(&path).await?;
        Self::init_reader(&reader).await?;

        let db = Self { writer, reader };
        db.ensure_dimensions().await?;
        Ok(db)
    }

    /// Open an in-memory database (for testing).
    pub async fn open_memory() -> Result<Self> {
        let writer = tokio_rusqlite::Connection::open_in_memory().await?;
        Self::init_writer(&writer).await?;

        // For in-memory, we share the same connection for reader/writer
        // since in-memory DBs are per-connection.
        let db = Self {
            reader: writer.clone(),
            writer,
        };
        db.ensure_dimensions().await?;
        Ok(db)
    }

    async fn init_writer(conn: &tokio_rusqlite::Connection) -> Result<()> {
        conn.call(|conn| {
            conn.execute_batch(
                "PRAGMA journal_mode=WAL;\
                 PRAGMA foreign_keys=ON;\
                 PRAGMA busy_timeout=5000;",
            )
            .map_err(|e| e.to_string())?;
            let migrations =
                Migrations::new(vec![
                    M::up(include_str!("migrations/001_initial.sql")),
                    M::up(include_str!("migrations/002_add_permalink_urls.sql")),
                ]);
            migrations
                .to_latest(conn)
                .map_err(|e| e.to_string())?;
            Ok::<(), String>(())
        })
        .await
        .map_err(|e| Error::Database(e.to_string()))
    }

    async fn init_reader(conn: &tokio_rusqlite::Connection) -> Result<()> {
        conn.call(|conn| {
            conn.execute_batch(
                "PRAGMA journal_mode=WAL;\
                 PRAGMA foreign_keys=ON;\
                 PRAGMA busy_timeout=5000;",
            )?;
            Ok::<(), rusqlite::Error>(())
        })
        .await?;
        Ok(())
    }

    /// Get a reference to the writer connection.
    pub fn writer(&self) -> &tokio_rusqlite::Connection {
        &self.writer
    }

    /// Get a reference to the reader connection.
    pub fn reader(&self) -> &tokio_rusqlite::Connection {
        &self.reader
    }

    /// Ensure dim_date and dim_period tables are populated.
    async fn ensure_dimensions(&self) -> Result<()> {
        self.writer
            .call(|conn| {
                schema::ensure_dim_date(conn)?;
                schema::ensure_dim_period(conn)?;
                Ok::<(), rusqlite::Error>(())
            })
            .await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_open_memory() {
        let db = Database::open_memory().await.unwrap();

        let tables: Vec<String> = db
            .reader()
            .call(|conn| {
                let mut stmt = conn.prepare(
                    "SELECT name FROM sqlite_master WHERE type='table' ORDER BY name",
                )?;
                let rows = stmt.query_map([], |row| row.get(0))?;
                Ok::<Vec<String>, rusqlite::Error>(rows.filter_map(|r| r.ok()).collect())
            })
            .await
            .unwrap();

        assert!(tables.contains(&"fact_tasks".to_string()));
        assert!(tables.contains(&"dim_users".to_string()));
        assert!(tables.contains(&"dim_projects".to_string()));
        assert!(tables.contains(&"dim_date".to_string()));
        assert!(tables.contains(&"dim_period".to_string()));
        assert!(tables.contains(&"app_config".to_string()));
        assert!(tables.contains(&"sync_jobs".to_string()));
    }

    #[tokio::test]
    async fn test_dim_date_populated() {
        let db = Database::open_memory().await.unwrap();

        let count: i64 = db
            .reader()
            .call(|conn| {
                Ok::<i64, rusqlite::Error>(
                    conn.query_row("SELECT COUNT(*) FROM dim_date", [], |row| row.get(0))?,
                )
            })
            .await
            .unwrap();

        assert!(count > 365, "dim_date should have >365 rows, got {count}");
    }

    #[tokio::test]
    async fn test_dim_period_populated() {
        let db = Database::open_memory().await.unwrap();

        let count: i64 = db
            .reader()
            .call(|conn| {
                Ok::<i64, rusqlite::Error>(
                    conn.query_row("SELECT COUNT(*) FROM dim_period", [], |row| row.get(0))?,
                )
            })
            .await
            .unwrap();

        assert!(count > 50, "dim_period should have >50 rows, got {count}");
    }
}
