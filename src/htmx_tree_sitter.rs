use std::{
    cell::RefCell,
    sync::{Arc, Mutex},
};

use dashmap::{mapref::one::Ref, DashMap};
use tree_sitter::{Parser, Tree};

use crate::init_hx::LangType;

#[derive(Debug)]
pub struct BackendTreeSitter {
    pub tree: Tree,
}

#[derive(Clone)]
pub struct LspFiles {
    current: RefCell<usize>,
    indexes: DashMap<String, usize>,
    trees: DashMap<usize, (Tree, LangType)>,
    parsers: Arc<Mutex<Parsers>>,
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
    pub fn add_tree(&self, index: usize, lang_type: Option<LangType>, text: &str) {
        // let language = match lang_type {
        //     LangType::Template => tree_sitter_rust::language(),
        //     LangType::JavaScript => tree_sitter_javascript::language(),
        //     LangType::Backend => tree_sitter_html::language(),
        // };

        let _ = self.parsers.lock().is_ok_and(|mut parsers| {
            if let Some(old_tree) = self.trees.get(&index) {
                if let Some(tree) = parsers.parse(old_tree.1, text, Some(&old_tree.0)) {
                    self.trees.insert(index, (tree, old_tree.1));
                }
            } else if let Some(lang_type) = lang_type {
                if let Some(tree) = parsers.parse(lang_type, text, None) {
                    self.trees.insert(index, (tree, lang_type));
                }
            }

            true
        });
    }

    pub fn get_tree(&self, index: usize) -> Option<Ref<'_, usize, (Tree, LangType)>> {
        self.trees.get(&index)
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
        old_tree: Option<&Tree>,
    ) -> Option<Tree> {
        //
        match lang_type {
            LangType::Template => self.html.parse(text, old_tree),
            LangType::JavaScript => self.javascript.parse(text, old_tree),
            LangType::Backend => self.backend.parse(text, old_tree),
        }
    }
}

impl Default for Parsers {
    fn default() -> Self {
        let mut html = Parser::new();
        let _ = html.set_language(tree_sitter_html::language());
        let mut javascript = Parser::new();
        let _ = javascript.set_language(tree_sitter_javascript::language());
        let mut backend = Parser::new();
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
