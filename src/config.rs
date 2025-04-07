use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// User configuration which can be persisted to disk.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Config {
    /// User's GroupMe API Token.
    /// WARN: Highly secret!!
    pub api_token: String,

    /// User's preferred base image download directory.
    pub image_dir: PathBuf,
}

impl Config {
    /// Create a new [`Config`] by supplying the `api_token`
    /// and prompting the user for a preferred `image_dir`.
    pub fn new(api_token: String) -> miette::Result<Self> {
        Ok(Self {
            api_token,
            image_dir: rfd::FileDialog::new()
                .pick_folder()
                .ok_or_else(|| miette::miette!("Must pick a target folder for image downloads."))?,
        })
    }
}
