use std::{
    cell::RefCell,
    collections::HashMap,
    sync::{Arc, Mutex},
};

use dashmap::{
    mapref::one::{Ref, RefMut},
    DashMap,
};
use tower_lsp::lsp_types::Range;
use tree_sitter::{InputEdit, Node, Parser, Point, Query, QueryCursor, QueryMatches, Tree};

use crate::{
    init_hx::LangType,
    position::CaptureDetails,
    queries::{HX_HTML, HX_JS_TAGS, HX_RUST_TAGS},
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
}

impl Default for LspFiles {
    fn default() -> Self {
        Self {
            current: RefCell::new(0),
            indexes: DashMap::new(),
            trees: DashMap::new(),
            parsers: Arc::new(Mutex::new(Parsers::default())),
        }
    }
}

impl LspFiles {
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

    /// LangType is None when it comes from editor.
    pub fn add_tree(
        &self,
        index: usize,
        lang_type: Option<LangType>,
        text: &str,
        range: Option<Range>,
    ) {
        let _ = self.parsers.lock().is_ok_and(|mut parsers| {
            if let Some(mut old_tree) = self.trees.get_mut(&index) {
                // old_tree.0.root_node().utf8_text()
                {
                    if let Some(range) = range {
                        //
                    }
                };
                // old_tree.0.edit(&input.unwrap());
                if let Some(tree) = parsers.parse(old_tree.1, text, Some(&old_tree.0)) {
                    let lang = old_tree.1;
                    drop(old_tree);
                    self.trees.insert(index, (tree, lang));
                }
            } else if let Some(lang_type) = lang_type {
                if let Some(tree) = parsers.parse(lang_type, text, None) {
                    self.trees.insert(index, (tree, lang_type));
                }
            }
            true
        });
    }

    pub fn add_tags(&self, index: usize, lang_type: LangType, text: &str, overwrite: bool) {
        if lang_type == LangType::Template {
            return;
        }
        let _ = self.parsers.lock().is_ok_and(|mut parsers| {
            if let Some(old_tree) = self.trees.get(&index) {
                parsers.query(
                    lang_type,
                    text,
                    old_tree.0.root_node(),
                    Point::default(),
                    true,
                );
            }
            true
        });
    }

    pub fn get_tree(&self, index: usize) -> Option<Ref<'_, usize, (Tree, LangType)>> {
        self.trees.get(&index)
    }

    pub fn get_mut_tree(&self, index: usize) -> Option<RefMut<'_, usize, (Tree, LangType)>> {
        self.trees.get_mut(&index)
    }
}

pub struct Parsers {
    html: (Parser, Query),
    javascript: (Parser, Query),
    backend: (Parser, Query),
}

impl Parsers {
    pub fn parse(
        &mut self,
        lang_type: LangType,
        text: &str,
        old_tree: Option<&Tree>,
    ) -> Option<Tree> {
        match lang_type {
            LangType::Template => self.html.0.parse(text, None),
            LangType::JavaScript => self.javascript.0.parse(text, None),
            LangType::Backend => self.backend.0.parse(text, None),
        }
    }

    pub fn query<'a, 'tree>(
        &'a self,
        lang_type: LangType,
        text: &'a str,
        node: Node<'tree>,
        trigger_point: Point,
        force: bool,
    ) -> HashMap<String, CaptureDetails> {
        let query: &'a Query = match lang_type {
            LangType::Template => &self.html.1,
            LangType::JavaScript => &self.javascript.1,
            LangType::Backend => &self.backend.1,
        };
        let mut cursor_qry = QueryCursor::new();
        let capture_names = query.capture_names();
        let matches = cursor_qry.matches(query, node, text.as_bytes());

        matches
            .into_iter()
            .flat_map(|m| {
                m.captures
                    .iter()
                    .filter(|capture| force || capture.node.start_position() <= trigger_point)
            })
            .fold(HashMap::new(), |mut acc, capture| {
                let key = capture_names[capture.index as usize].to_owned();
                let value = if let Ok(capture_value) = capture.node.utf8_text(text.as_bytes()) {
                    capture_value.to_owned()
                } else {
                    "".to_owned()
                };

                acc.insert(
                    key,
                    CaptureDetails {
                        value,
                        end_position: capture.node.end_position(),
                    },
                );

                acc
            })

        // ) -> QueryMatches<'a, 'tree, T> {
    }
}

impl Default for Parsers {
    fn default() -> Self {
        let mut html = Parser::new();
        let query_html = Query::new(tree_sitter_html::language(), HX_HTML).unwrap();
        let _ = html.set_language(tree_sitter_html::language());
        let mut javascript = Parser::new();
        let query_javascript = Query::new(tree_sitter_javascript::language(), HX_JS_TAGS).unwrap();
        let _ = javascript.set_language(tree_sitter_javascript::language());
        let mut backend = Parser::new();
        let query_backend = Query::new(tree_sitter_rust::language(), HX_RUST_TAGS).unwrap();
        let _ = backend.set_language(tree_sitter_rust::language());

        Self {
            html: (html, query_html),
            javascript: (javascript, query_javascript),
            backend: (backend, query_backend),
        }
    }
}

impl Clone for Parsers {
    fn clone(&self) -> Self {
        Self::default()
    }
}

// fn from_range_to_input(range: &Range) -> InputEdit {
//     // let start_byte = range.start.line
//     // InputEdit { start_byte: , old_end_byte: , new_end_byte: , start_position: , old_end_position: , new_end_position:  }
//     //
// }
