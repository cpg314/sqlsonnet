// TODO: There might be an easier way of doing this...
use super::*;

#[derive(Trace)]
pub struct Resolver {
    inner: jrsonnet_evaluator::FileImportResolver,
    utils: Vec<u8>,
}
impl Resolver {
    pub fn new(paths: ImportPaths) -> Self {
        Self {
            inner: jrsonnet_evaluator::FileImportResolver::new(paths.0),
            utils: include_bytes!("../../utils.libsonnet").into(),
        }
    }
}
impl jrsonnet_evaluator::ImportResolver for Resolver {
    fn resolve_from(
        &self,
        from: &SourcePath,
        path: &str,
    ) -> jrsonnet_evaluator::Result<SourcePath> {
        if path == UTILS_FILENAME {
            return Ok(SourcePath::new(jrsonnet_parser::SourceFile::new(
                UTILS_FILENAME.into(),
            )));
        }
        self.inner.resolve_from(from, path)
    }
    fn resolve_from_default(&self, path: &str) -> jrsonnet_evaluator::Result<SourcePath> {
        self.inner.resolve_from_default(path)
    }
    fn resolve(&self, path: &Path) -> jrsonnet_evaluator::Result<SourcePath> {
        self.inner.resolve(path)
    }
    fn load_file_contents(&self, resolved: &SourcePath) -> jrsonnet_evaluator::Result<Vec<u8>> {
        if resolved
            .path()
            .map_or(false, |p| p == Path::new(UTILS_FILENAME))
        {
            return Ok(self.utils.clone());
        }
        self.inner.load_file_contents(resolved)
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

#[derive(Clone, Default)]
pub struct ImportPaths(Vec<PathBuf>);
impl ImportPaths {
    // Produces statements of the form
    // local file_stem = import 'path.libsonnet';
    pub fn imports(&self) -> String {
        self.0
            .iter()
            .map(|f| glob::glob(f.join("*.libsonnet").to_str().unwrap()))
            .filter_map(|glob| glob.ok())
            .flatten()
            .filter_map(|f| f.ok())
            .map(|f| {
                format!(
                    "local {} = import '{}';",
                    f.file_stem().unwrap().to_string_lossy(),
                    f.file_name().unwrap().to_string_lossy()
                )
            })
            .chain(std::iter::once(format!(
                "local u = import '{}';",
                UTILS_FILENAME
            )))
            .join("\n")
    }
}
impl<P: Into<PathBuf>> From<P> for ImportPaths {
    fn from(source: P) -> Self {
        Self(vec![source.into()])
    }
}
