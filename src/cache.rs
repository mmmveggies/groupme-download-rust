use std::{
    fs::{self, File},
    path::{Path, PathBuf},
};

#[cfg(not(windows))]
use std::os::unix::fs::PermissionsExt;

use miette::IntoDiagnostic;
use serde::{Deserialize, Serialize};

use crate::config::Config;

/// A helper for caching [`Config`] and frequently used items.
#[derive(Clone)]
pub struct Cache {
    cache_dir: PathBuf,
    config_dir: PathBuf,
}

impl Cache {
    /// Create a [`Cache`] ensuring that necessary directories are created,
    /// and that we can write files to them/remove files from them.
    pub fn new() -> miette::Result<Self> {
        const APP_DIRNAME: &str = "groupme_downloader";

        let cache_dir = dirs::cache_dir()
            .map(|dir| dir.join(APP_DIRNAME))
            .ok_or_else(|| miette::miette!("Unable to locate user's cache directory."))?;

        let config_dir = dirs::config_dir()
            .map(|dir| dir.join(APP_DIRNAME))
            .ok_or_else(|| miette::miette!("Unable to locate user's config directory."))?;

        for dir in [&cache_dir, &config_dir] {
            if !fs::exists(dir).into_diagnostic()? {
                std::fs::create_dir(dir).into_diagnostic()?;
            }
            let test_file = dir.join(".test_file");
            std::fs::write(&test_file, "").into_diagnostic()?;
            std::fs::remove_file(test_file).into_diagnostic()?;
        }

        Ok(Self {
            cache_dir,
            config_dir,
        })
    }

    // -- config

    fn config_file_path(&self) -> PathBuf {
        const APP_CONFIG_FILENAME: &str = "config.json";
        self.config_dir.join(APP_CONFIG_FILENAME)
    }

    /// Get the [`Config`] from disk, if one exists.
    /// To persist any config changes to disk, use [`Self::write_config`].
    pub fn read_config(&self) -> miette::Result<Option<Config>> {
        let filepath = &self.config_file_path();
        read_json(filepath)
    }

    /// Persist the [`Config`] to disk, and ensures the correct file mode is set.
    pub fn write_config(&self, config: &Config) -> miette::Result<()> {
        let filepath = &self.config_file_path();
        write_json(filepath, config)
    }

    // -- cache

    /// Read a cached file as JSON, if it exists.
    pub fn read_cache_item<T>(&self, filename: impl AsRef<Path>) -> miette::Result<Option<T>>
    where
        for<'de> T: Deserialize<'de>,
    {
        let filepath = &self.cache_dir.join(filename.as_ref());
        read_json(filepath)
    }

    /// Write a file to the cache directory, overwriting it if it exists.
    pub fn write_cache_item<T>(&self, filename: impl AsRef<Path>, data: &T) -> miette::Result<()>
    where
        T: Serialize,
    {
        let filepath = &self.cache_dir.join(filename.as_ref());
        write_json(filepath, data)
    }
}

/// Read JSON from a file and deserialize as `T`, if the file exists.
fn read_json<T>(filepath: &PathBuf) -> miette::Result<Option<T>>
where
    for<'de> T: Deserialize<'de>,
{
    if !fs::exists(filepath).into_diagnostic()? {
        return Ok(None);
    }

    let reader = File::open(filepath).into_diagnostic()?;
    let data: Result<T, _> =
        serde_path_to_error::deserialize(&mut serde_json::Deserializer::from_reader(reader))
            .into_diagnostic();

    Some(data).transpose()
}

/// Write `data` as JSON to a file, overwriting if the file exists.
fn write_json<T>(filepath: &PathBuf, data: &T) -> miette::Result<()>
where
    T: Serialize,
{
    let file = File::options()
        .create(true)
        .write(true)
        .truncate(true)
        .open(filepath)
        .into_diagnostic()?;

    let mut permissions = file.metadata().into_diagnostic()?.permissions();

    #[cfg(not(windows))]
    permissions.set_mode(0o600);

    fs::set_permissions(filepath, permissions).into_diagnostic()?;

    serde_path_to_error::serialize(data, &mut serde_json::Serializer::pretty(file))
        .into_diagnostic()
}
