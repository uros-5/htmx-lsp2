use tree_sitter::Point;

#[derive(Debug)]
struct Tag<'a> {
    data: &'a str,
    start: usize,
    end: usize,
}

pub fn find_tag(tags: &str, start: usize, _end: usize, point: Point) -> Option<String> {
    let parts = tags.split(" ");
    let parts = parts.into_iter();

    let mut iter_start = start;
    let mut tags = vec![];
    for part in parts {
        let start = iter_start + 1;
        iter_start += part.len();
        let end = iter_start;
        let tag = Tag {
            start: start.try_into().unwrap(),
            end: end.try_into().unwrap(),
            data: part,
        };
        tags.push(tag);
    }
    let filtered: Vec<_> = tags
        .iter()
        .filter(|c| {
            if point.column >= c.start && point.column < c.end {
                return true;
            }
            false
        })
        .collect();
    if let Some(filtered) = filtered.first() {
        return Some(String::from(filtered.data));
    }
    None
}
