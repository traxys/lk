use anyhow::Result;
use serde::{Deserialize, Serialize};

use std::{
    fs::OpenOptions,
    io::{BufRead, BufReader, BufWriter, Write},
    path::Path,
};

#[derive(Serialize, Deserialize)]
pub struct Config {
    /// The default mode: fuzzy or list
    pub default_mode: String,
}

pub struct ConfigFile {
    pub config: Config,
    lk_dir: String,
    file_name: String,
}

impl ConfigFile {
    pub fn new(lk_dir: &str, file_name: &str) -> Self {
        let path = format!("{}/{}", lk_dir, file_name);
        // Create a default config file if it doesn't exist
        if !Path::new(&path).exists() {
            log::info!("Creating config file at {}", path);
            match OpenOptions::new().write(true).create(true).open(&path) {
                Ok(mut file) => {
                    let mut buffered = BufWriter::new(file);
                    let default_config = Config {
                        default_mode: "list".to_string(),
                    };
                    let toml = toml::to_string(&default_config).unwrap();
                    write!(buffered, "{}", toml);
                }
                Err(e) => log::error!("Unable to create default config file: {}", e),
            }
        } else {
            log::info!("Using config file at {}", path);
        }

        // Load the config file
        let config_string = std::fs::read_to_string(path).expect("Couldn't read config file");
        let config = toml::from_str::<Config>(&config_string).expect("Couldn't parse config file");
        Self {
            config,
            lk_dir: lk_dir.to_string(),
            file_name: file_name.to_string(),
        }
    }

    pub fn save(&self) {
        let path = format!("{}/{}", self.lk_dir, self.file_name);
        let toml = toml::to_string(&self.config).expect("Couldn't serialize config file");
        let mut file = OpenOptions::new()
            .write(true)
            .create(true)
            .open(&path)
            .expect(format!("Couldn't open config file at {}", path).as_str());
        let mut buffered = BufWriter::new(file);
        write!(buffered, "{}", toml).expect("Couldn't write to config file");
    }

    // pub fn set_default_mode(&self, mode: &str) -> Result<()> {
    //     // let path = format!("{}/llk.toml", lk_dir);
    //     let mut file = OpenOptions::new().write(true).open(path)?;
    //     let mut buffered = BufWriter::new(file);
    //     let default_config = Config {
    //         default_mode: mode.to_string(),
    //     };
    //     let toml = toml::to_string(&default_config).unwrap();
    //     write!(buffered, "{}", toml);
    //     Ok(())
    // }
}

// fn save_default_mode(path: &str, default_mode: &str) -> Result<()> {
//     let file = OpenOptions::new().write(true).create(true).open(path)?;
//     let mut buffered = BufWriter::new(file);
//     let default_config = Config {
//         default_mode: default_mode.to_string(),
//     };
//     let toml = toml::to_string(&default_config).unwrap();
//     write!(buffered, "{}", toml)?;
//     Ok(())
// }
