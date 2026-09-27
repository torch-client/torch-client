use std::collections::HashSet;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub struct Range {
    pub start: usize,
    pub end: usize,
}

impl Range {
    pub fn at(pos: usize) -> Range {
        Range {
            start: pos,
            end: pos,
        }
    }

    pub fn between(start: usize, end: usize) -> Range {
        Range { start, end }
    }

    pub(super) fn encompassing(a: Range, b: Range) -> Range {
        Range {
            start: a.start.min(b.start),
            end: a.end.max(b.end),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.start == self.end
    }
}

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct Suggestion {
    pub range: Range,
    pub text: String,
    pub tooltip: Option<String>,
}

impl Suggestion {
    pub fn apply(&self, input: &[char]) -> String {
        let start = self.range.start.min(input.len());
        let end = self.range.end.min(input.len());
        let mut out: String = input[..start].iter().collect();
        out.push_str(&self.text);
        out.extend(&input[end..]);
        out
    }

    fn expand(&self, command: &[char], range: Range) -> Suggestion {
        if range == self.range {
            return self.clone();
        }
        let mut text = String::new();
        if range.start < self.range.start {
            text.extend(&command[range.start..self.range.start.min(command.len())]);
        }
        text.push_str(&self.text);
        if range.end > self.range.end {
            text.extend(&command[self.range.end.min(command.len())..range.end.min(command.len())]);
        }
        Suggestion {
            range,
            text,
            tooltip: self.tooltip.clone(),
        }
    }
}

#[derive(Clone, Default, Debug, PartialEq, Eq)]
pub struct Suggestions {
    pub range: Range,
    pub list: Vec<Suggestion>,
}

impl Suggestions {
    pub fn is_empty(&self) -> bool {
        self.list.is_empty()
    }

    pub fn create(command: &[char], input: Vec<Suggestion>) -> Suggestions {
        if input.is_empty() {
            return Suggestions::default();
        }
        let start = input.iter().map(|s| s.range.start).min().unwrap();
        let end = input.iter().map(|s| s.range.end).max().unwrap();
        let range = Range { start, end };
        let mut seen: HashSet<Suggestion> = HashSet::new();
        let mut list: Vec<Suggestion> = Vec::new();
        for s in input {
            let expanded = s.expand(command, range);
            if seen.insert(expanded.clone()) {
                list.push(expanded);
            }
        }
        list.sort_by(|a, b| {
            a.text
                .to_lowercase()
                .cmp(&b.text.to_lowercase())
                .then_with(|| a.text.cmp(&b.text))
        });
        Suggestions { range, list }
    }

    pub fn merge(command: &[char], input: Vec<Suggestions>) -> Suggestions {
        let mut all: Vec<Suggestion> = Vec::new();
        for s in input {
            all.extend(s.list);
        }
        Suggestions::create(command, all)
    }
}
