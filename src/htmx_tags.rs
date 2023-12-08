
use tree_sitter::Point;

use crate::position::PositionDefinition;

#[derive(Debug, Clone)]
pub struct Tag {
    pub name: String,
    pub start: usize,
    pub end: usize,
    pub file: usize,
    pub line: usize,
}

impl Tag {
    pub fn set_file(&mut self, index: usize) {
        self.file = index;
    }

    pub fn set_line(&mut self, line: usize) {
        self.line = line;
    }
}

pub fn in_tag(line: &str, point: Point) -> Option<Tag> {
    if let Some(tag) = get_tag(line) {
        if point.column >= tag.start && point.column <= tag.end {
            return Some(tag);
        }
    }
    None
}

pub fn get_tag(line: &str) -> Option<Tag> {
    let parts = line.split("hx@");
    let mut first = parts.filter(|data| !data.contains(' '));
    if let Some(first) = first.next() {
        let mut parts = first.split(' ');
        if let Some(first) = parts.next() {
            let full = format!("hx@{}", &first);
            if let Some(start) = line.find(&full) {
                let end = start + 2 + first.len();
                return Some(Tag {
                    name: first.to_string(),
                    start,
                    end,
                    file: 0,
                    line: 0,
                });
            }
        }
    }
    None
}

pub fn get_tags(value: &str, mut start_char: usize, line: usize) -> Option<Vec<Tag>> {
    if value.starts_with(' ') || value.contains("  ") {
        return None;
    }
    let mut tags = vec![];
    let parts = value.split(' ');
    for part in parts {
        let start = start_char;
        let end = start + part.len() - 1;
        start_char = end + 2;
        let tag = Tag {
            name: String::from(part),
            start,
            end,
            file: 0,
            line,
        };
        tags.push(tag);
    }
    Some(tags)
}

pub fn in_tags(value: &str, definition: PositionDefinition) -> Option<Tag> {
    if let Some(tags) = get_tags(value, definition.start, definition.line) {
        for tag in tags {
            let t = definition.point.column >= tag.start && definition.point.column <= tag.end;
            if t {
                return Some(tag);
            }
        }
    }
    None
}
