use std::fmt;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct Position {
    pub(crate) line: u32,
    pub(crate) column: u32,
}

impl Position {
    pub(crate) fn of(text: &str, offset: usize) -> Position {
        let offset = offset.min(text.len());
        let before = &text[..floor_boundary(text, offset)];
        let line = before.bytes().filter(|&b| b == b'\n').count() as u32 + 1;
        let start = before.rfind('\n').map_or(0, |i| i + 1);
        let column = before[start..].chars().count() as u32 + 1;
        Position { line, column }
    }
}

fn floor_boundary(text: &str, mut offset: usize) -> usize {
    while !text.is_char_boundary(offset) {
        offset -= 1;
    }
    offset
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Problem {
    pub(crate) file: String,
    pub(crate) at: Option<Position>,
    pub(crate) message: String,
    pub(crate) hint: Option<String>,
}

impl Problem {
    pub(crate) fn package(message: impl Into<String>) -> Problem {
        Problem {
            file: String::new(),
            at: None,
            message: message.into(),
            hint: None,
        }
    }

    pub(crate) fn file(file: impl Into<String>, message: impl Into<String>) -> Problem {
        Problem {
            file: file.into(),
            at: None,
            message: message.into(),
            hint: None,
        }
    }

    pub(crate) fn at(
        file: impl Into<String>,
        text: &str,
        offset: usize,
        message: impl Into<String>,
    ) -> Problem {
        Problem {
            file: file.into(),
            at: Some(Position::of(text, offset)),
            message: message.into(),
            hint: None,
        }
    }

    pub(crate) fn hint(mut self, hint: impl Into<String>) -> Problem {
        self.hint = Some(hint.into());
        self
    }
}

const SUMMARY_SHOWN: usize = 3;

pub(crate) fn summary(problems: &[Problem]) -> String {
    let mut text = problems
        .iter()
        .take(SUMMARY_SHOWN)
        .map(Problem::to_string)
        .collect::<Vec<_>>()
        .join("; ");
    if problems.len() > SUMMARY_SHOWN {
        text.push_str(&format!("; and {} more", problems.len() - SUMMARY_SHOWN));
    }
    text
}

impl fmt::Display for Problem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match (self.file.is_empty(), self.at) {
            (true, _) => write!(f, "{}", self.message)?,
            (false, None) => write!(f, "{}: {}", self.file, self.message)?,
            (false, Some(p)) => {
                write!(f, "{}:{}:{}: {}", self.file, p.line, p.column, self.message)?
            }
        }
        if let Some(hint) = &self.hint {
            write!(f, " ({hint})")?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn positions_count_lines_and_characters() {
        let text = "ab\ncé d\n";
        assert_eq!(Position::of(text, 0), Position { line: 1, column: 1 });
        assert_eq!(Position::of(text, 3), Position { line: 2, column: 1 });
        assert_eq!(Position::of(text, 6), Position { line: 2, column: 3 });
        assert_eq!(Position::of(text, 999), Position { line: 3, column: 1 });
    }

    #[test]
    fn a_problem_prints_as_file_line_column() {
        let p = Problem::at("a.wgsl", "x\ny", 2, "bad").hint("fix");
        assert_eq!(p.to_string(), "a.wgsl:2:1: bad (fix)");
        assert_eq!(Problem::package("too big").to_string(), "too big");
    }

    #[test]
    fn a_summary_names_three_and_counts_the_rest() {
        let problems: Vec<Problem> = (0..5).map(|n| Problem::package(format!("p{n}"))).collect();
        assert_eq!(summary(&problems), "p0; p1; p2; and 2 more");
        assert_eq!(summary(&problems[..1]), "p0");
    }
}
