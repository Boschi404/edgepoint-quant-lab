use qs_core::*;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CatalogRunRecord {
    pub run_id: RunId,
    pub state: PersistentRunState,
    pub created_at: i64,
    pub updated_at: i64,
    pub pipeline_version: String,
    pub seed: u64,
    pub metadata: RunMetadata,
}

pub struct RunCatalog {
    conn: Connection,
}

impl RunCatalog {
    pub fn open(path: &Path) -> Result<Self, StorageError> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| StorageError::Message {
                code: "CATALOG_MKDIR".into(),
                message: e.to_string(),
                retryable: true,
            })?;
        }
        let conn = Connection::open(path).map_err(sql_err("CATALOG_OPEN"))?;
        conn.pragma_update(None, "journal_mode", "WAL")
            .map_err(sql_err("CATALOG_WAL"))?;
        conn.pragma_update(None, "foreign_keys", "ON")
            .map_err(sql_err("CATALOG_FK"))?;
        conn.execute_batch(include_str!("../migrations/001_init.sql"))
            .map_err(sql_err("CATALOG_MIGRATE"))?;
        Ok(Self { conn })
    }

    pub fn upsert_run(&self, record: &CatalogRunRecord) -> Result<(), StorageError> {
        let metadata_json =
            serde_json::to_string(&record.metadata).map_err(|e| StorageError::Message {
                code: "CATALOG_METADATA_SERIALIZE".into(),
                message: e.to_string(),
                retryable: false,
            })?;
        self.conn.execute(
            "INSERT INTO runs(run_id,state,created_at,updated_at,pipeline_version,seed,metadata_json)
             VALUES(?1,?2,?3,?4,?5,?6,?7)
             ON CONFLICT(run_id) DO UPDATE SET state=excluded.state, updated_at=excluded.updated_at,
             pipeline_version=excluded.pipeline_version, seed=excluded.seed, metadata_json=excluded.metadata_json",
            params![&record.run_id.0, state_to_str(&record.state), record.created_at, record.updated_at, &record.pipeline_version, record.seed as i64, metadata_json],
        ).map_err(sql_err("CATALOG_UPSERT_RUN"))?;
        Ok(())
    }

    pub fn set_state(
        &self,
        run_id: &RunId,
        state: PersistentRunState,
        updated_at: i64,
    ) -> Result<(), StorageError> {
        self.conn
            .execute(
                "UPDATE runs SET state=?1, updated_at=?2 WHERE run_id=?3",
                params![state_to_str(&state), updated_at, &run_id.0],
            )
            .map_err(sql_err("CATALOG_SET_STATE"))?;
        Ok(())
    }

    pub fn get_run(&self, run_id: &RunId) -> Result<Option<CatalogRunRecord>, StorageError> {
        self.conn.query_row(
            "SELECT run_id,state,created_at,updated_at,pipeline_version,seed,metadata_json FROM runs WHERE run_id=?1",
            params![&run_id.0],
            row_to_record,
        ).optional().map_err(sql_err("CATALOG_GET_RUN"))
    }

    pub fn list_runs(&self, limit: usize) -> Result<Vec<CatalogRunRecord>, StorageError> {
        let mut stmt = self.conn.prepare(
            "SELECT run_id,state,created_at,updated_at,pipeline_version,seed,metadata_json FROM runs ORDER BY created_at DESC LIMIT ?1"
        ).map_err(sql_err("CATALOG_PREPARE_LIST"))?;
        let rows = stmt
            .query_map(params![limit as i64], row_to_record)
            .map_err(sql_err("CATALOG_QUERY_LIST"))?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row.map_err(sql_err("CATALOG_ROW_LIST"))?);
        }
        Ok(out)
    }

    pub fn mark_running_as_interrupted(&self, now: i64) -> Result<usize, StorageError> {
        self.conn
            .execute(
                "UPDATE runs SET state='Interrupted', updated_at=?1 WHERE state='Running'",
                params![now],
            )
            .map_err(sql_err("CATALOG_MARK_INTERRUPTED"))
    }
}

fn row_to_record(row: &rusqlite::Row<'_>) -> rusqlite::Result<CatalogRunRecord> {
    let metadata_json: String = row.get(6)?;
    let metadata: RunMetadata = match serde_json::from_str(&metadata_json) {
        Ok(value) => value,
        Err(_) => RunMetadata::default(),
    };
    let state_s: String = row.get(1)?;
    Ok(CatalogRunRecord {
        run_id: RunId(row.get(0)?),
        state: str_to_state(&state_s),
        created_at: row.get(2)?,
        updated_at: row.get(3)?,
        pipeline_version: row.get(4)?,
        seed: row.get::<_, i64>(5)? as u64,
        metadata,
    })
}

fn state_to_str(state: &PersistentRunState) -> &'static str {
    match state {
        PersistentRunState::Running => "Running",
        PersistentRunState::Paused => "Paused",
        PersistentRunState::Interrupted => "Interrupted",
        PersistentRunState::Completed => "Completed",
        PersistentRunState::Failed => "Failed",
    }
}

fn str_to_state(value: &str) -> PersistentRunState {
    match value {
        "Running" => PersistentRunState::Running,
        "Paused" => PersistentRunState::Paused,
        "Completed" => PersistentRunState::Completed,
        "Failed" => PersistentRunState::Failed,
        _ => PersistentRunState::Interrupted,
    }
}

fn sql_err(code: &'static str) -> impl Fn(rusqlite::Error) -> StorageError {
    move |e| StorageError::Message {
        code: code.into(),
        message: e.to_string(),
        retryable: true,
    }
}
