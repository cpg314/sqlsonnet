use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use tokio::sync::Mutex;

use super::SqlQuery;

#[derive(thiserror::Error, Debug)]
pub enum CacheError {
    #[error("I/O error: {0}")]
    IO(#[from] std::io::Error),
}

#[allow(dead_code)]
pub struct Cache {
    path: PathBuf,
    entries: Mutex<HashMap<SqlQuery, Arc<Mutex<()>>>>,
}
impl Cache {
    pub fn init(path: &Path) -> Result<Self, CacheError> {
        std::fs::create_dir(path)?;
        Ok(Self {
            path: path.into(),
            entries: Default::default(),
        })
    }
}
