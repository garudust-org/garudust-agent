use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use rusqlite::Connection;

use crate::migrations;

pub struct SessionDb {
    conn: Arc<Mutex<Connection>>,
}

impl SessionDb {
    pub fn open(home_dir: &PathBuf) -> anyhow::Result<Self> {
        let db_path = home_dir.join("state.db");
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(&db_path)?;
        migrations::run(&conn)?;
        Ok(Self { conn: Arc::new(Mutex::new(conn)) })
    }

    pub fn search(&self, query: &str, limit: usize) -> anyhow::Result<Vec<String>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT content FROM messages_fts WHERE messages_fts MATCH ?1 LIMIT ?2"
        )?;
        let rows = stmt.query_map([query, &limit.to_string()], |row| row.get(0))?;
        rows.collect::<Result<Vec<String>, _>>().map_err(Into::into)
    }
}
