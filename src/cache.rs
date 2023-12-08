use std::{path::PathBuf, time::Duration};

use error_stack::{Report, ResultExt};
use serde::{de::DeserializeOwned, Serialize};

use crate::error::Error;

#[derive(Debug)]
pub struct Cache {
    dir: PathBuf,
}

impl Cache {
    pub fn new() -> Result<Self, Report<Error>> {
        let dir = dirs::cache_dir()
            .ok_or(Error::Cache)
            .attach_printable("platform has no cache directory")?
            .join("promptbox");

        std::fs::create_dir_all(&dir)
            .change_context(Error::Cache)
            .attach_printable_lazy(|| format!("Creating cache directory {}", dir.display()))?;

        Ok(Self { dir })
    }

    /// Read a file from the cache.
    pub fn read_cache<T: DeserializeOwned>(
        &self,
        filename: &str,
        max_stale: Duration,
    ) -> Result<Option<T>, Report<Error>> {
        let path = self.dir.join(filename);
        let file = std::fs::File::open(&path)
            .change_context(Error::Cache)
            .attach_printable_lazy(|| format!("{}", path.display()))?;

        let meta = file
            .metadata()
            .change_context(Error::Cache)
            .attach_printable_lazy(|| format!("{}", path.display()))?;

        if meta.modified().unwrap().elapsed().unwrap_or(Duration::MAX) > max_stale {
            return Ok(None);
        }

        let reader = std::io::BufReader::new(file);
        serde_json::from_reader(reader)
            .change_context(Error::Cache)
            .attach_printable_lazy(|| format!("{}", path.display()))
    }

    pub fn write_cache(&self, filename: &str, data: impl Serialize) -> Result<(), Report<Error>> {
        let path = self.dir.join(filename);
        let file = std::fs::File::create(&path)
            .change_context(Error::Cache)
            .attach_printable_lazy(|| format!("Creating file {}", path.display()))?;
        serde_json::to_writer(file, &data)
            .change_context(Error::Cache)
            .attach_printable_lazy(|| format!("Writing file {}", path.display()))?;
        Ok(())
    }
}
