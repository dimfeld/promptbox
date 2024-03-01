use std::{path::PathBuf, time::Duration};

use error_stack::{Report, ResultExt};
use etcetera::BaseStrategy;
use serde::{de::DeserializeOwned, Serialize};

use crate::error::Error;

#[derive(Debug)]
pub struct Cache {
    dir: PathBuf,
}

impl Cache {
    pub fn new() -> Result<Self, Report<Error>> {
        let etc = etcetera::base_strategy::choose_native_strategy().unwrap();
        let dir = etc.cache_dir().join("promptbox");

        std::fs::create_dir_all(&dir)
            .change_context(Error::Cache)
            .attach_printable_lazy(|| format!("Creating cache directory {}", dir.display()))?;

        Ok(Self { dir })
    }

    /// Read a file from the cache if it's not older than `max_stale`.
    pub fn read_cache<T: DeserializeOwned>(
        &self,
        filename: &str,
        max_stale: Duration,
    ) -> Result<Option<T>, Report<Error>> {
        let path = self.dir.join(filename);
        let file = match std::fs::File::open(&path) {
            Ok(file) => file,
            Err(e) => {
                if e.kind() == std::io::ErrorKind::NotFound {
                    return Ok(None);
                } else {
                    return Err(e)
                        .change_context(Error::Cache)
                        .attach_printable_lazy(|| format!("{}", path.display()));
                }
            }
        };

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

    // Write a file to the cache as JSON
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

#[cfg(test)]
mod test {
    use std::time::Duration;

    use super::Cache;

    #[test]
    fn cache() {
        let dir = tempfile::tempdir().unwrap();
        let cache = Cache {
            dir: dir.path().to_path_buf(),
        };

        cache
            .write_cache("test.json", "hello")
            .expect("write_cache");
        let written = cache
            .read_cache("test.json", Duration::from_secs(100000))
            .expect("read_cache");

        assert_eq!(written, Some("hello".to_string()));

        let empty: Option<String> = cache
            .read_cache("test.json", Duration::from_secs(0))
            .unwrap();

        assert!(empty.is_none());
    }

    #[test]
    fn file_doesnt_exist() {
        let temp_dir = tempfile::tempdir().unwrap();
        let cache = Cache {
            dir: temp_dir.path().to_path_buf(),
        };

        let result: Option<String> = cache
            .read_cache("test.json", Duration::from_secs(0))
            .unwrap();
        assert!(result.is_none());
    }
}
