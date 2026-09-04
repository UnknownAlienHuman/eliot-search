//! Concrete redb control-store adapter for the development daemon composition.
//!
//! The store persists only daemon lifecycle metadata. Source bodies, queries,
//! excerpts, vectors, credentials, and unrestricted paths are never written.

use std::path::{Path, PathBuf};

use redb::{Database, ReadableTable, TableDefinition};

const STATE_TABLE: TableDefinition<&str, &str> =
    TableDefinition::new("eliot_search_daemon_state_v1");
const SCHEMA_VERSION: &str = "1";

pub(crate) struct DevelopmentControlStore {
    database: Database,
    database_path: PathBuf,
}

impl DevelopmentControlStore {
    pub(crate) fn open(data_root: &Path) -> Result<Self, String> {
        let database_path = data_root.join("control.redb");
        let database = Database::create(&database_path)
            .map_err(|_| "CONTROL_STORE_OPEN_FAILED".to_owned())?;
        let store = Self {
            database,
            database_path,
        };
        store.write_lifecycle("ACTIVE")?;
        store.verify_lifecycle("ACTIVE")?;
        Ok(store)
    }

    pub(crate) fn mark_stopped(&self) -> Result<(), String> {
        self.write_lifecycle("STOPPED")?;
        self.verify_lifecycle("STOPPED")
    }

    pub(crate) fn path(&self) -> &Path {
        &self.database_path
    }

    fn write_lifecycle(&self, lifecycle: &str) -> Result<(), String> {
        let transaction = self
            .database
            .begin_write()
            .map_err(|_| "CONTROL_STORE_WRITE_BEGIN_FAILED".to_owned())?;
        {
            let mut table = transaction
                .open_table(STATE_TABLE)
                .map_err(|_| "CONTROL_STORE_TABLE_OPEN_FAILED".to_owned())?;
            if let Some(schema) = table
                .get("schema")
                .map_err(|_| "CONTROL_STORE_SCHEMA_READ_FAILED".to_owned())?
            {
                if schema.value() != SCHEMA_VERSION {
                    return Err("CONTROL_STORE_SCHEMA_MISMATCH".to_owned());
                }
            }
            table
                .insert("schema", SCHEMA_VERSION)
                .map_err(|_| "CONTROL_STORE_SCHEMA_WRITE_FAILED".to_owned())?;
            table
                .insert("lifecycle", lifecycle)
                .map_err(|_| "CONTROL_STORE_LIFECYCLE_WRITE_FAILED".to_owned())?;
            let pid = std::process::id().to_string();
            table
                .insert("pid", pid.as_str())
                .map_err(|_| "CONTROL_STORE_PID_WRITE_FAILED".to_owned())?;
        }
        transaction
            .commit()
            .map_err(|_| "CONTROL_STORE_COMMIT_FAILED".to_owned())
    }

    fn verify_lifecycle(&self, expected: &str) -> Result<(), String> {
        let transaction = self
            .database
            .begin_read()
            .map_err(|_| "CONTROL_STORE_READ_BEGIN_FAILED".to_owned())?;
        let table = transaction
            .open_table(STATE_TABLE)
            .map_err(|_| "CONTROL_STORE_TABLE_READ_FAILED".to_owned())?;
        let schema = table
            .get("schema")
            .map_err(|_| "CONTROL_STORE_SCHEMA_READ_FAILED".to_owned())?
            .ok_or_else(|| "CONTROL_STORE_SCHEMA_MISSING".to_owned())?;
        if schema.value() != SCHEMA_VERSION {
            return Err("CONTROL_STORE_SCHEMA_MISMATCH".to_owned());
        }
        let lifecycle = table
            .get("lifecycle")
            .map_err(|_| "CONTROL_STORE_LIFECYCLE_READ_FAILED".to_owned())?
            .ok_or_else(|| "CONTROL_STORE_LIFECYCLE_MISSING".to_owned())?;
        if lifecycle.value() != expected {
            return Err("CONTROL_STORE_READBACK_MISMATCH".to_owned());
        }
        Ok(())
    }
}
