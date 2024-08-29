use std::sync::Arc;

use super::*;

/// A simpler version of [`jrsonnet_evaluator::ImportResolver`], so that we can
/// easily implement it on `Arc<T>`.
pub trait ImportResolver: Trace + Sized {
    fn resolve(&self, from: &SourcePath, path: &str) -> Option<PathBuf>;
    fn load(&self, resolved: &SourcePath) -> Result<Vec<u8>, std::io::Error>;
    /// This adds the resolution of the embedded sqlsonnet.libsonnet import.
    fn to_resolver(self) -> impl jrsonnet_evaluator::ImportResolver {
        ResolverWrapper {
            inner: self,
            utils: include_bytes!("../../sqlsonnet.libsonnet").to_vec(),
        }
    }
}
impl<T: ImportResolver> ImportResolver for Arc<T> {
    fn resolve(&self, from: &SourcePath, path: &str) -> Option<PathBuf> {
        self.as_ref().resolve(from, path)
    }
    fn load(&self, resolved: &SourcePath) -> Result<Vec<u8>, std::io::Error> {
        self.as_ref().load(resolved)
    }
}

/// Simple filesystem resolver
#[derive(Trace)]
pub struct FsResolver {
    search_paths: Vec<PathBuf>,
}
impl Default for FsResolver {
    fn default() -> Self {
        Self::new(vec![])
    }
}
impl FsResolver {
    pub fn current_dir() -> Self {
        Self::new(std::env::current_dir().map(|d| vec![d]).unwrap_or_default())
    }
    pub fn add(&mut self, path: PathBuf) {
        self.search_paths.push(path);
    }
    pub fn new(search_paths: Vec<PathBuf>) -> Self {
        Self { search_paths }
    }
}
impl ImportResolver for FsResolver {
    fn resolve(&self, _from: &SourcePath, path: &str) -> Option<PathBuf> {
        // Search paths
        if let Some(p) = self
            .search_paths
            .iter()
            .map(|p| {
                let p = p.clone();
                p.join(path)
            })
            .find(|p| p.exists())
        {
            return Some(p);
        }
        None
    }
    fn load(&self, resolved: &SourcePath) -> Result<Vec<u8>, std::io::Error> {
        let path = resolved.path().unwrap();
        std::fs::read(path)
    }
}
#[derive(Trace)]
struct ResolverWrapper<T: Trace + 'static> {
    inner: T,
    utils: Vec<u8>,
}
impl<T: ImportResolver + Trace + 'static> jrsonnet_evaluator::ImportResolver
    for ResolverWrapper<T>
{
    fn resolve_from(
        &self,
        from: &SourcePath,
        path: &str,
    ) -> jrsonnet_evaluator::Result<SourcePath> {
        if path == UTILS_FILENAME {
            return Ok(SourcePath::new(jrsonnet_parser::SourceVirtual(path.into())));
        }
        if let Some(path) = self.inner.resolve(from, path) {
            return Ok(SourcePath::new(jrsonnet_parser::SourceFile::new(path)));
        }
        Err(jrsonnet_evaluator::error::ErrorKind::ImportFileNotFound(
            from.clone(),
            path.to_string(),
        )
        .into())
    }
    fn load_file_contents(&self, resolved: &SourcePath) -> jrsonnet_evaluator::Result<Vec<u8>> {
        if resolved
            .downcast_ref::<jrsonnet_parser::SourceVirtual>()
            .map_or(false, |p| p.0 == *UTILS_FILENAME)
        {
            return Ok(self.utils.clone());
        }
        self.inner
            .load(resolved)
            .map_err(|e| jrsonnet_evaluator::error::ErrorKind::ImportIo(e.to_string()).into())
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}
