use dashmap::DashMap;
use ropey::Rope;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    fs::read_to_string,
    path::Path,
    sync::{Arc, Mutex, MutexGuard, RwLock},
};

use crate::{
    htmx_tags::Tag,
    htmx_tree_sitter::LspFiles,
    init_hx::{LangType, LangTypes},
    query_helper::Queries,
};

/// Help language server by providing additional info about your htmx project.
#[derive(Serialize, Deserialize, Debug, Default)]
pub struct HtmxConfig {
    /// Backend language for htmx project.
    pub lang: String,
    /// Template language file extension. It can be only of one type(for example jinja).
    pub template_ext: String,
    /// List of directories for templates, it must contain relative paths.
    /// ```json
    /// { "templates": ["./templates"] }
    /// ````
    /// Language server searches only for `template_ext` file extension.
    pub templates: Vec<String>,
    /// List of directories for JavaScript/TypeScript, it must contain relative paths.
    /// ```json
    /// { "js_tags": ["./frontend/src/htmx_part"] }
    /// ````
    /// Language server searches for `js/ts` file extension.
    pub js_tags: Vec<String>,
    /// List of directories for selected backend language, it must contain relative paths.
    /// ```json
    /// { "backend_tags": ["./src"] }
    /// ````
    /// Language server searches for proper backend file extension.
    pub backend_tags: Vec<String>,
    #[serde(skip)]
    /// This field is not serializable/deserializable.
    /// Every LSP request supported by HtmxBackend first checks if config is valid
    /// (hover and completion works without checks).
    pub is_valid: bool,
}

impl HtmxConfig {
    /// Check if passed file extension is in client config.
    pub fn file_ext(&self, path: &Path) -> Option<LangTypes> {
        match path.extension()?.to_str() {
            Some(e) => match e {
                "js" | "ts" => Some(LangTypes::One(LangType::JavaScript)),
                other => {
                    if self.is_backend(other) {
                        match self.template_ext == other {
                            true => Some(LangTypes::two((LangType::Backend, LangType::Template))),
                            false => Some(LangTypes::one(LangType::Backend)),
                        }
                    } else if other == self.template_ext {
                        Some(LangTypes::one(LangType::Template))
                    } else {
                        None
                    }
                }
            },
            None => None,
        }
    }
    /// Checks if passed file extension is supported backend.
    pub fn is_backend(&self, ext: &str) -> bool {
        match self.lang.as_str() {
            "rust" => ext == "rs",
            "python" => ext == "py",
            "go" => ext == "go",
            _ => false,
        }
    }

    pub fn is_supported_backend(&self) -> bool {
        matches!(self.lang.as_str(), "python" | "rust" | "go")
    }
}

/// Quickly check config on initialization request.
pub fn validate_config(config: Option<Value>) -> Option<HtmxConfig> {
    if let Some(config) = config {
        if let Ok(mut config) = serde_json::from_value::<HtmxConfig>(config) {
            config.is_valid = true;
            return Some(config);
        }
    }
    None
}

/// Read config. Language server can be used even if config
/// haven't passed all checks
pub fn read_config(
    config: &RwLock<HtmxConfig>,
    lsp_files: &Arc<Mutex<LspFiles>>,
    queries: &Arc<Mutex<Queries>>,
    document_map: &DashMap<String, Rope>,
) -> anyhow::Result<Vec<Tag>> {
    if let Ok(config) = config.read() {
        if config.template_ext.is_empty() || config.template_ext.contains(' ') {
            return Err(anyhow::Error::msg("Template extension not found."));
        } else if !config.is_supported_backend() {
            return Err(anyhow::Error::msg(format!(
                "Language {} is not supported.",
                config.lang
            )));
        }
        walkdir(&config, lsp_files, queries, document_map)
    } else {
        Err(anyhow::Error::msg("Config is not found"))
    }
}

/// Walk through all directories and files. In this process it catches all
/// duplicated tag errors.
fn walkdir(
    config: &HtmxConfig,
    lsp_files: &Arc<Mutex<LspFiles>>,
    queries: &Arc<Mutex<Queries>>,
    document_map: &DashMap<String, Rope>,
) -> anyhow::Result<Vec<Tag>> {
    let lsp_files = lsp_files.lock().unwrap();
    let mut diagnostics = vec![];
    lsp_files.reset();
    let directories = [&config.templates, &config.js_tags, &config.backend_tags];
    queries
        .lock()
        .ok()
        .and_then(|mut queries| queries.change_backend(&config.lang));
    for (index, dir) in directories.iter().enumerate() {
        let lang_type = LangType::from(index);
        lsp_files
            .parsers
            .lock()
            .ok()
            .and_then(|mut parsers| parsers.change_backend(&config.lang, lang_type));
        for file in dir.iter() {
            for entry in walkdir::WalkDir::new(file) {
                let entry = entry?;
                let metadata = entry.metadata()?;
                if metadata.is_file() {
                    let path = &entry.path();
                    let ext = config.file_ext(path);
                    if !ext.is_some_and(|ext| ext.is_lang(lang_type)) {
                        continue;
                    }
                    if queries
                        .lock()
                        .ok()
                        .and_then(|queries| {
                            add_file(
                                path,
                                &lsp_files,
                                lang_type,
                                &queries,
                                &mut diagnostics,
                                false,
                                document_map,
                            )
                        })
                        .is_none()
                    {
                        return Err(anyhow::Error::msg(format!(
                            "Template path: {} does not exist",
                            file
                        )));
                    };
                }
            }
        }
    }
    Ok(diagnostics)
}

/// Get path, read contents of file, parse TreeSitter tree and check for tags.
fn add_file(
    path: &&Path,
    lsp_files: &MutexGuard<LspFiles>,
    lang_type: LangType,
    queries: &Queries,
    diags: &mut Vec<Tag>,
    _skip: bool,
    document_map: &DashMap<String, Rope>,
) -> Option<bool> {
    if let Ok(name) = std::fs::canonicalize(path) {
        let name = name.to_str()?;
        let file = lsp_files.add_file(format!("file://{}", name))?;
        return read_to_string(name).ok().map(|content| {
            let rope = ropey::Rope::from_str(&content);
            document_map.insert(format!("file://{}", name).to_string(), rope);
            lsp_files.add_tree(file, lang_type, &content, None);
            let _ = lsp_files.add_tags_from_file(file, lang_type, &content, false, queries, diags);
            true
        });
    }
    None
}
