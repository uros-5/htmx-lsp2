use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    fs::read_to_string,
    path::Path,
    sync::{Arc, Mutex, MutexGuard, RwLock},
};

use crate::{htmx_tree_sitter::LspFiles, init_hx::LangType};
use thiserror::Error;

#[derive(Serialize, Deserialize, Debug)]
pub struct HtmxConfig {
    pub lang: String,
    pub template_ext: String,
    pub templates: Vec<String>,
    pub js_tags: Vec<String>,
    pub backend_tags: Vec<String>,
}

impl HtmxConfig {
    pub fn ext(&self, ext: &str) -> bool {
        match self.lang.as_str() {
            "rust" => ext == "rs",
            _ => false,
        }
    }
}

pub fn validate_config(config: Option<Value>) -> Option<HtmxConfig> {
    if let Some(config) = config {
        if let Ok(config) = serde_json::from_value::<HtmxConfig>(config) {
            return Some(config);
        }
    }
    None
}

pub fn read_config(
    config: &RwLock<Option<HtmxConfig>>,
    lsp_files: &Arc<Mutex<LspFiles>>,
) -> Result<(), ConfigError> {
    if let Ok(config) = config.read() {
        if let Some(config) = config.as_ref().filter(|_| true) {
            if config.template_ext.is_empty() || config.template_ext.contains(' ') {
                return Err(ConfigError::TemplateExtension);
            } else if config.lang != "rust" {
                return Err(ConfigError::LanguageSupport(String::from(&config.lang)));
            }
            walkdir(config, lsp_files)
        } else {
            Err(ConfigError::ConfigNotFound)
        }
    } else {
        Err(ConfigError::ConfigNotFound)
    }
}

fn walkdir(config: &HtmxConfig, lsp_files: &Arc<Mutex<LspFiles>>) -> Result<(), ConfigError> {
    let lsp_files = lsp_files.lock().unwrap();
    let directories = [&config.templates, &config.js_tags, &config.backend_tags];
    for (index, dir) in directories.iter().enumerate() {
        let lang_type = LangType::from(index);
        for file in dir.iter() {
            for entry in walkdir::WalkDir::new(file) {
                if let Ok(entry) = &entry {
                    if let Ok(metadata) = &entry.metadata() {
                        if metadata.is_file() {
                            let path = &entry.path();
                            let ext = file_ext(path, lang_type, config);
                            if !ext {
                                continue;
                            }
                            add_file(path, &lsp_files, lang_type);
                        }
                    }
                } else {
                    return Err(ConfigError::TemplatePath(String::from(file)));
                }
            }
        }
    }
    Ok(())
}

fn file_ext(path: &Path, lang_type: LangType, config: &HtmxConfig) -> bool {
    path.extension().is_some_and(|x| {
        return match x.to_str() {
            Some(e) => {
                return match e {
                    "js" | "ts" => lang_type == LangType::JavaScript,
                    backend if config.ext(backend) => lang_type == LangType::Backend,
                    template if template == config.template_ext => lang_type == LangType::Template,
                    _ => false,
                };
            }
            None => false,
        };
    })
}

fn add_file(path: &&Path, lsp_files: &MutexGuard<LspFiles>, lang_type: LangType) {
    if let Ok(name) = std::fs::canonicalize(path) {
        if let Some(name) = name.to_str() {
            if let Some(index) = lsp_files.add_file(format!("file://{}", name)) {
                let _ = read_to_string(name).is_ok_and(|f| {
                    lsp_files.add_tree(index, Some(lang_type), &f, None);
                    lsp_files.add_tags(index, lang_type, &f, false);
                    true
                });
            }
        }
    }
}

#[derive(Error, Debug)]
pub enum ConfigError {
    #[error("Template path: {0} does not exist")]
    TemplatePath(String),
    #[error("Language {0} is not supported")]
    LanguageSupport(String),
    #[error("Template extension is empty")]
    TemplateExtension,
    #[error("Config is not found")]
    ConfigNotFound,
}
