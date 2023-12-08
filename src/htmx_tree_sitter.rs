use std::{
    cell::RefCell,
    collections::HashMap,
    path::Path,
    sync::{Arc, Mutex, RwLock},
};

use dashmap::{
    mapref::one::{Ref, RefMut},
    DashMap,
};
use ropey::Rope;
use tower_lsp::lsp_types::{
    Diagnostic, DiagnosticSeverity, GotoDefinitionResponse, Location, Position, Range, Url,
};
use tree_sitter::{Parser, Point, Query, Tree};

use crate::{
    config::{file_ext, HtmxConfig},
    htmx_tags::{in_tags, Tag},
    init_hx::LangType,
    position::{PositionDefinition, QueryType},
    queries::{HX_HTML, HX_JS_TAGS, HX_NAME, HX_RUST_TAGS, HX_VALUE},
    query_helper::{query_tag, HtmxQuery, Queries},
    server::{LocalWriter, ServerTextDocumentItem},
};

#[derive(Debug)]
pub struct BackendTreeSitter {
    pub tree: Tree,
}

#[derive(Clone)]
pub struct LspFiles {
    current: RefCell<usize>,
    indexes: DashMap<String, usize>,
    trees: DashMap<usize, (Tree, LangType)>,
    pub parsers: Arc<Mutex<Parsers>>,
    pub tags: DashMap<String, Tag>,
}

impl Default for LspFiles {
    fn default() -> Self {
        Self {
            current: RefCell::new(0),
            indexes: DashMap::new(),
            trees: DashMap::new(),
            parsers: Arc::new(Mutex::new(Parsers::default())),
            tags: DashMap::new(),
        }
    }
}

impl LspFiles {
    pub fn reset(&self) {
        self.indexes.clear();
        self.trees.clear();
        self.tags.clear();
    }

    pub fn delete_tags_by_index(&self, index: usize) {
        let mut tags = vec![];
        for i in &self.tags {
            let file = i.value().file;
            if file == index {
                tags.push(String::from(i.key()));
            }
        }
        for i in tags {
            self.tags.remove(&i);
        }
    }

    pub fn add_tag(&self, tag: Tag) -> Result<(), Tag> {
        if self.tags.contains_key(&tag.name) {
            Err(tag)
        } else {
            self.tags.insert(String::from(&tag.name), tag);
            Ok(())
        }
    }

    pub fn get_tag<'a>(&'a self, key: &String) -> Option<Ref<'a, std::string::String, Tag>> {
        self.tags.get(key)
    }

    pub fn add_file(&self, key: String) -> Option<usize> {
        if self.get_index(&key).is_none() {
            let old = self.current.replace_with(|&mut old| old + 1);
            self.indexes.insert(key, old);
            return Some(old);
        }
        None
    }

    pub fn get_index(&self, key: &String) -> Option<usize> {
        if let Some(d) = self.indexes.get(key) {
            let a = *d;
            return Some(a);
        }
        None
    }

    pub fn get_uri(&self, index: usize) -> Option<String> {
        self.indexes.iter().find_map(|item| {
            if item.value() == &index {
                Some(String::from(item.key()))
            } else {
                None
            }
        })
    }

    pub fn on_change(&self, params: ServerTextDocumentItem) -> Option<()> {
        let file = self.get_index(&params.uri.to_string())?;
        self.add_tree(file, None, &params.text, None);
        None
    }

    pub fn publish_tag_diagnostics(
        &self,
        diagnostics: Vec<Tag>,
        hm: &mut HashMap<String, Vec<Diagnostic>>,
    ) {
        for diag in diagnostics {
            if let Some(uri) = self.get_uri(diag.file) {
                let diagnostic = Diagnostic {
                    range: Range::new(
                        Position::new(diag.line as u32, diag.start as u32),
                        Position::new(diag.line as u32, diag.end as u32),
                    ),
                    severity: Some(DiagnosticSeverity::WARNING),
                    message: String::from("This tag already exist."),
                    source: Some(String::from("htmx-lsp")),
                    ..Default::default()
                };
                if hm.contains_key(&uri) {
                    let _ = hm.get_mut(&uri).is_some_and(|d| {
                        d.push(diagnostic);
                        false
                    });
                } else {
                    hm.insert(String::from(&uri), vec![diagnostic]);
                }
            }
        }
    }

    pub fn goto_definition_response(
        &self,
        definition: Option<PositionDefinition>,
        value: &str,
        def: &mut Option<GotoDefinitionResponse>,
    ) -> Option<()> {
        let tag = in_tags(value, definition?)?;
        let tag = self.get_tag(&tag.name)?;
        let file = self.get_uri(tag.file)?;
        let start = Position::new(tag.line as u32, tag.start as u32);
        let end = Position::new(tag.line as u32, tag.end as u32);
        let range = Range::new(start, end);
        *def = Some(GotoDefinitionResponse::Scalar(Location {
            uri: Url::parse(&file).unwrap(),
            range,
        }));
        None
    }

    /// LangType is None when it comes from editor.
    pub fn add_tree(
        &self,
        index: usize,
        lang_type: Option<LangType>,
        text: &str,
        _range: Option<Range>,
    ) {
        let _ = self.parsers.lock().is_ok_and(|mut parsers| {
            if let Some(old_tree) = self.trees.get_mut(&index) {
                if let Some(tree) = parsers.parse(old_tree.1, text, Some(&old_tree.0)) {
                    let lang = old_tree.1;
                    drop(old_tree);
                    self.trees.insert(index, (tree, lang));
                }
            } else if let Some(lang_type) = lang_type {
                // tree doesn't exist, first insertion
                if let Some(tree) = parsers.parse(lang_type, text, None) {
                    self.trees.insert(index, (tree, lang_type));
                }
            }
            true
        });
    }

    pub fn add_tags_from_file(
        &self,
        index: usize,
        lang_type: LangType,
        text: &str,
        _overwrite: bool,
        queries: &Queries,
        diags: &mut Vec<Tag>,
    ) -> Result<(), ()> {
        let query = HtmxQuery::try_from(lang_type)?;
        let query = queries.get(query);
        if let Some(old_tree) = self.trees.get(&index) {
            let tags = query_tag(
                old_tree.0.root_node(),
                text,
                Point::new(0, 0),
                &QueryType::Completion,
                query,
                true,
            );
            self.delete_tags_by_index(index);
            for mut tag in tags {
                tag.set_file(index);
                if let Err(tag) = self.add_tag(tag) {
                    diags.push(tag);
                }
            }
            drop(old_tree);
        }
        Ok(())
    }

    pub fn saved(
        &self,
        uri: &String,
        diagnostics: &mut Vec<Tag>,
        config: &RwLock<Option<HtmxConfig>>,
        document_map: &DashMap<String, Rope>,
        queries: &Queries,
    ) -> Option<Vec<Tag>> {
        let path = Path::new(&uri);
        let file = self.get_index(uri)?;
        if let Ok(config) = config.read() {
            let config = config.as_ref()?;
            let lang_type = file_ext(path, config)?;
            if lang_type == LangType::Template {
                return None;
            }
            let _ext = file_ext(path, config)?;
            let content = document_map.get(uri)?;
            let content = content.value();
            let mut a = LocalWriter::default();
            let _ = content.write_to(&mut a);
            let content = a.content;
            let _ = self.add_tags_from_file(file, lang_type, &content, false, queries, diagnostics);
            return Some(diagnostics.to_vec());
            //
        }
        None
    }

    pub fn get_tree(&self, index: usize) -> Option<Ref<'_, usize, (Tree, LangType)>> {
        self.trees.get(&index)
    }

    pub fn get_mut_tree(&self, index: usize) -> Option<RefMut<'_, usize, (Tree, LangType)>> {
        self.trees.get_mut(&index)
    }
}

pub struct Parsers {
    html: Parser,
    javascript: Parser,
    backend: Parser,
}

impl Parsers {
    pub fn parse(
        &mut self,
        lang_type: LangType,
        text: &str,
        _old_tree: Option<&Tree>,
    ) -> Option<Tree> {
        match lang_type {
            LangType::Template => self.html.parse(text, None),
            LangType::JavaScript => self.javascript.parse(text, None),
            LangType::Backend => self.backend.parse(text, None),
        }
    }
}

impl Default for Parsers {
    fn default() -> Self {
        let mut html = Parser::new();
        let _ = html.set_language(tree_sitter_html::language());
        let mut javascript = Parser::new();
        let _query_javascript = Query::new(tree_sitter_javascript::language(), HX_JS_TAGS).unwrap();
        let _ = javascript.set_language(tree_sitter_javascript::language());
        let mut backend = Parser::new();
        let _query_backend = Query::new(tree_sitter_rust::language(), HX_RUST_TAGS).unwrap();
        let _ = backend.set_language(tree_sitter_rust::language());

        Self {
            html,
            javascript,
            backend,
        }
    }
}

impl Clone for Parsers {
    fn clone(&self) -> Self {
        Self::default()
    }
}

pub struct HTMLQueries {
    lsp: Query,
    name: Query,
    value: Query,
}

impl Default for HTMLQueries {
    fn default() -> Self {
        let lsp = Query::new(tree_sitter_html::language(), HX_HTML).unwrap();
        let name = Query::new(tree_sitter_html::language(), HX_NAME).unwrap();
        let value = Query::new(tree_sitter_html::language(), HX_VALUE).unwrap();
        Self { lsp, name, value }
    }
}

pub enum HTMLQuery {
    Lsp,
    Name,
    Value,
}
