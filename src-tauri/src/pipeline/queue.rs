//! Durable SQLite task queue (rusqlite, sync) — the live operational store the pipeline
//! runner drives. Chosen over async job-queue crates (`effectum` is tokio-based) because the
//! runner is sync + off-reactor so `reqwest::blocking` works; rusqlite is sync and the
//! claim/complete/fail/retry surface is small enough to own. Durable: survives restart, and on
//! open any task stuck `claimed` (worker died mid-step) is reset to `pending` so the run
//! resumes. The DB file lives in the app data dir (never the vault).
// Consumed by the runner (Task 4) + app (Task 6); suppress dead-code until wired.
#![allow(dead_code)]

use rusqlite::{params, Connection, OptionalExtension};
use std::path::Path;
use std::sync::Mutex;
use std::time::Duration;

/// Maximum attempts for non-scrape stages (LLM, script) before marking a task dead.
pub const MAX_ATTEMPTS: u32 = 3;

/// Maximum attempts for transient scrape failures before the run is marked failed.
/// Kept lower than `MAX_ATTEMPTS` because each scrape attempt costs credits.
pub const TRANSIENT_SCRAPE_MAX_ATTEMPTS: u32 = 2;

#[derive(Debug, Clone, PartialEq)]
pub struct NewTask {
    pub run_id: String,
    pub stage: String,
    pub class: String,
    pub target: String,
    pub payload: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct QueuedTask {
    pub id: i64,
    pub run_id: String,
    pub stage: String,
    pub class: String,
    pub target: String,
    pub payload: String,
    pub attempts: u32,
}

pub trait Queue {
    fn enqueue(&self, t: NewTask) -> Result<i64, String>;
    /// Claim the oldest pending task: marks it `claimed`, increments `attempts`, returns it.
    fn claim_next(&self) -> Result<Option<QueuedTask>, String>;
    fn complete(&self, id: i64) -> Result<(), String>;
    /// Re-queue (`pending`) if `attempts < MAX_ATTEMPTS`, else mark `dead`; records the error.
    fn fail(&self, id: i64, err: &str) -> Result<(), String>;
    /// Mark a task `dead` immediately regardless of attempt count; records the error.
    /// Used for Terminal scrape failures and one-shot escalations that have already been
    /// re-enqueued as a separate task.
    fn kill(&self, id: i64, err: &str) -> Result<(), String>;
    fn pending_count(&self) -> Result<usize, String>;
}

/// Exponential backoff between retry attempts, capped at 5 minutes.
pub fn backoff_delay(attempt: u32) -> Duration {
    Duration::from_secs(2u64.saturating_pow(attempt).min(300))
}

pub struct SqliteQueue {
    conn: Mutex<Connection>,
}

impl SqliteQueue {
    pub fn open(path: &Path) -> Result<Self, String> {
        let conn = Connection::open(path).map_err(|e| e.to_string())?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS tasks (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                run_id TEXT NOT NULL,
                stage TEXT NOT NULL,
                class TEXT NOT NULL,
                target TEXT NOT NULL,
                payload TEXT NOT NULL,
                attempts INTEGER NOT NULL DEFAULT 0,
                state TEXT NOT NULL DEFAULT 'pending',
                last_error TEXT
            );
            UPDATE tasks SET state='pending' WHERE state='claimed';",
        )
        .map_err(|e| e.to_string())?;
        Ok(SqliteQueue { conn: Mutex::new(conn) })
    }

    fn lock(&self) -> Result<std::sync::MutexGuard<'_, Connection>, String> {
        self.conn.lock().map_err(|e| e.to_string())
    }
}

impl Queue for SqliteQueue {
    fn enqueue(&self, t: NewTask) -> Result<i64, String> {
        let conn = self.lock()?;
        conn.execute(
            "INSERT INTO tasks (run_id, stage, class, target, payload) VALUES (?1,?2,?3,?4,?5)",
            params![t.run_id, t.stage, t.class, t.target, t.payload],
        )
        .map_err(|e| e.to_string())?;
        Ok(conn.last_insert_rowid())
    }

    fn claim_next(&self) -> Result<Option<QueuedTask>, String> {
        let conn = self.lock()?;
        let row = conn
            .query_row(
                "SELECT id, run_id, stage, class, target, payload, attempts
                 FROM tasks WHERE state='pending' ORDER BY id LIMIT 1",
                [],
                |r| {
                    let attempts: i64 = r.get(6)?;
                    Ok(QueuedTask {
                        id: r.get(0)?,
                        run_id: r.get(1)?,
                        stage: r.get(2)?,
                        class: r.get(3)?,
                        target: r.get(4)?,
                        payload: r.get(5)?,
                        attempts: attempts as u32,
                    })
                },
            )
            .optional()
            .map_err(|e| e.to_string())?;
        let Some(mut task) = row else {
            return Ok(None);
        };
        conn.execute(
            "UPDATE tasks SET state='claimed', attempts=attempts+1 WHERE id=?1",
            params![task.id],
        )
        .map_err(|e| e.to_string())?;
        task.attempts += 1;
        Ok(Some(task))
    }

    fn complete(&self, id: i64) -> Result<(), String> {
        self.lock()?
            .execute("UPDATE tasks SET state='done' WHERE id=?1", params![id])
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    fn fail(&self, id: i64, err: &str) -> Result<(), String> {
        self.lock()?
            .execute(
                "UPDATE tasks
                 SET state = CASE WHEN attempts < ?1 THEN 'pending' ELSE 'dead' END,
                     last_error = ?2
                 WHERE id = ?3",
                params![MAX_ATTEMPTS, err, id],
            )
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    fn kill(&self, id: i64, err: &str) -> Result<(), String> {
        self.lock()?
            .execute(
                "UPDATE tasks SET state='dead', last_error=?1 WHERE id=?2",
                params![err, id],
            )
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    fn pending_count(&self) -> Result<usize, String> {
        let n: i64 = self
            .lock()?
            .query_row("SELECT COUNT(*) FROM tasks WHERE state='pending'", [], |r| r.get(0))
            .map_err(|e| e.to_string())?;
        Ok(n as usize)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    static SEQ: AtomicU32 = AtomicU32::new(0);
    fn temp_db() -> std::path::PathBuf {
        let n = SEQ.fetch_add(1, Ordering::SeqCst);
        std::env::temp_dir().join(format!("lodestar-queue-{}-{}.db", std::process::id(), n))
    }
    fn new(run: &str, stage: &str) -> NewTask {
        NewTask {
            run_id: run.into(),
            stage: stage.into(),
            class: "scrape".into(),
            target: "stripe".into(),
            payload: "{}".into(),
        }
    }

    #[test]
    fn enqueue_claim_complete_cycle() {
        let db = temp_db();
        let q = SqliteQueue::open(&db).unwrap();
        q.enqueue(new("r1", "careers-scrape")).unwrap();
        assert_eq!(q.pending_count().unwrap(), 1);
        let t = q.claim_next().unwrap().unwrap();
        assert_eq!(t.stage, "careers-scrape");
        assert_eq!(t.attempts, 1);
        q.complete(t.id).unwrap();
        assert_eq!(q.pending_count().unwrap(), 0);
        assert!(q.claim_next().unwrap().is_none());
        std::fs::remove_file(&db).ok();
    }

    #[test]
    fn fail_retries_until_max_then_stops() {
        let db = temp_db();
        let q = SqliteQueue::open(&db).unwrap();
        let id = q.enqueue(new("r1", "careers-scrape")).unwrap();
        for _ in 0..MAX_ATTEMPTS {
            let t = q.claim_next().unwrap().expect("still claimable");
            assert_eq!(t.id, id);
            q.fail(t.id, "boom").unwrap();
        }
        assert!(
            q.claim_next().unwrap().is_none(),
            "exhausted retries -> not re-claimed"
        );
        std::fs::remove_file(&db).ok();
    }

    #[test]
    fn pending_tasks_survive_reopen() {
        let db = temp_db();
        {
            let q = SqliteQueue::open(&db).unwrap();
            q.enqueue(new("r1", "careers-scrape")).unwrap();
        }
        let q2 = SqliteQueue::open(&db).unwrap(); // fresh handle, same file
        assert_eq!(q2.pending_count().unwrap(), 1);
        assert!(q2.claim_next().unwrap().is_some());
        std::fs::remove_file(&db).ok();
    }

    #[test]
    fn claimed_task_is_requeued_on_reopen() {
        let db = temp_db();
        {
            let q = SqliteQueue::open(&db).unwrap();
            q.enqueue(new("r1", "careers-scrape")).unwrap();
            q.claim_next().unwrap().unwrap(); // claimed but neither completed nor failed
            assert_eq!(q.pending_count().unwrap(), 0);
        }
        // Reopen: a stuck-`claimed` task (worker died) is re-queued so the run resumes.
        let q2 = SqliteQueue::open(&db).unwrap();
        assert_eq!(q2.pending_count().unwrap(), 1);
        std::fs::remove_file(&db).ok();
    }

    #[test]
    fn backoff_grows_with_attempts() {
        assert!(backoff_delay(2) > backoff_delay(1));
    }
}
