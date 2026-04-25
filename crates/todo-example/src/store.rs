//! Tiny JSON-backed todo store. Mutex inside, `&self` outside, so handlers
//! can stay simple and `app_state` can hand out a shared reference.
//!
//! In a real app you'd put a SQLite DB or a service client here; the shape
//! is the same — handlers receive a `&Store` from `ctx.app_state` and call
//! straightforward methods on it.

use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Todo {
    pub id: u32,
    pub title: String,
    pub done: bool,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
struct Inner {
    todos: Vec<Todo>,
    next_id: u32,
}

pub struct TodoStore {
    path: PathBuf,
    inner: Mutex<Inner>,
}

impl TodoStore {
    pub fn load(path: PathBuf) -> Result<Self> {
        let inner = if path.exists() {
            let raw = std::fs::read_to_string(&path)
                .with_context(|| format!("reading {}", path.display()))?;
            serde_json::from_str(&raw).with_context(|| format!("parsing {}", path.display()))?
        } else {
            Inner::default()
        };
        Ok(Self {
            path,
            inner: Mutex::new(inner),
        })
    }

    pub fn list(&self) -> Vec<Todo> {
        self.lock().todos.clone()
    }

    pub fn add(&self, title: String) -> Result<Todo> {
        let mut g = self.lock();
        // Snapshot-then-commit: build the new state into `next` and only
        // swap it in after the disk write succeeds, so a save failure
        // can't leave the in-memory store ahead of the file.
        let mut next = g.clone();
        next.next_id += 1;
        let todo = Todo {
            id: next.next_id,
            title,
            done: false,
        };
        next.todos.push(todo.clone());
        save(&self.path, &next)?;
        *g = next;
        Ok(todo)
    }

    pub fn mark_done(&self, id: u32) -> Result<Todo> {
        let mut g = self.lock();
        let mut next = g.clone();
        let t = next
            .todos
            .iter_mut()
            .find(|t| t.id == id)
            .with_context(|| format!("no todo with id {}", id))?;
        t.done = true;
        let snapshot = t.clone();
        save(&self.path, &next)?;
        *g = next;
        Ok(snapshot)
    }

    fn lock(&self) -> MutexGuard<'_, Inner> {
        // The mutex only guards in-process state; CLI runs are single-
        // threaded, so poisoning shouldn't happen in normal operation.
        // If it does, surface a clear message rather than a bare panic.
        self.inner
            .lock()
            .expect("TodoStore mutex poisoned — this is a bug")
    }
}

fn save(path: &Path, inner: &Inner) -> Result<()> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("creating directory {}", parent.display()))?;
        }
    }
    let json = serde_json::to_string_pretty(inner)
        .map_err(|e| anyhow!(e))
        .context("serializing store")?;
    std::fs::write(path, json).with_context(|| format!("writing {}", path.display()))?;
    Ok(())
}
