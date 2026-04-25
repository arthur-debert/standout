//! Tiny JSON-backed todo store. Mutex inside, `&self` outside, so handlers
//! can stay simple and `app_state` can hand out a shared reference.
//!
//! In a real app you'd put a SQLite DB or a service client here; the shape
//! is the same — handlers receive a `&Store` from `ctx.app_state` and call
//! straightforward methods on it.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Todo {
    pub id: u32,
    pub title: String,
    pub done: bool,
}

#[derive(Debug, Default, Serialize, Deserialize)]
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
        self.inner.lock().unwrap().todos.clone()
    }

    pub fn add(&self, title: String) -> Result<Todo> {
        let mut g = self.inner.lock().unwrap();
        g.next_id += 1;
        let todo = Todo {
            id: g.next_id,
            title,
            done: false,
        };
        g.todos.push(todo.clone());
        save(&self.path, &g)?;
        Ok(todo)
    }

    pub fn mark_done(&self, id: u32) -> Result<Todo> {
        let mut g = self.inner.lock().unwrap();
        let t = g
            .todos
            .iter_mut()
            .find(|t| t.id == id)
            .with_context(|| format!("no todo with id {}", id))?;
        t.done = true;
        let snapshot = t.clone();
        save(&self.path, &g)?;
        Ok(snapshot)
    }
}

fn save(path: &Path, inner: &Inner) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).ok();
    }
    let json = serde_json::to_string_pretty(inner)?;
    std::fs::write(path, json).with_context(|| format!("writing {}", path.display()))?;
    Ok(())
}
