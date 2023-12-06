#[cfg(test)]
mod test_tree_sitter {

    use crate::{htmx_tree_sitter::Parsers, init_hx::LangType};

    #[test]
    pub fn rust_tags() {
        let mut parsers = Parsers::default();
        let rust_s = r#"
        fn main() {
            // hx@something
            let msg = "hello";
        }
           "#;
        // let parsed = parsers.parse(LangType::Backend, rust_s, None).unwrap();
        // let query = parsers.query(LangType::Backend, rust_s, parsed.root_node());
        assert!(false);
    }
}

type Primer = dyn FnOnce() -> usize;
