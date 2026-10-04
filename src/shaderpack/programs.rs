use super::options::{Kind, Options, Values};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum Geometry {
    TerrainSolid,
    TerrainCutout,
    Water,
}

impl Geometry {
    pub(crate) const ALL: [Geometry; 3] = [
        Geometry::TerrainSolid,
        Geometry::TerrainCutout,
        Geometry::Water,
    ];

    pub(crate) fn chain(self) -> &'static [&'static str] {
        const TAIL: [&str; 4] = [
            "gbuffers_terrain",
            "gbuffers_textured_lit",
            "gbuffers_textured",
            "gbuffers_basic",
        ];
        match self {
            Geometry::TerrainSolid => {
                &["gbuffers_terrain_solid", TAIL[0], TAIL[1], TAIL[2], TAIL[3]]
            }
            Geometry::TerrainCutout => &[
                "gbuffers_terrain_cutout",
                TAIL[0],
                TAIL[1],
                TAIL[2],
                TAIL[3],
            ],
            Geometry::Water => &["gbuffers_water", TAIL[0], TAIL[1], TAIL[2], TAIL[3]],
        }
    }

    pub(crate) fn translucent(self) -> bool {
        self == Geometry::Water
    }
}

pub(crate) fn resolve(geometry: Geometry, has: impl Fn(&str) -> bool) -> Option<&'static str> {
    geometry.chain().iter().copied().find(|name| has(name))
}

pub(crate) const LAST_PASS: u32 = 99;

pub(crate) fn enabled<'a>(
    program: &str,
    expressions: impl Iterator<Item = (&'a str, &'a str)>,
    options: &Options,
    values: &Values,
) -> bool {
    expressions
        .filter(|(name, _)| *name == program)
        .all(|(_, expression)| evaluate(expression, options, values))
}

pub(crate) fn enable_expressions<'a>(
    keys: impl Iterator<Item = (&'a String, &'a String)>,
) -> impl Iterator<Item = (&'a str, &'a str)> {
    keys.filter_map(|(key, value)| {
        let rest = key.strip_prefix("program.")?;
        let program = &rest[..rest.find('.')?];
        Some((program, value.as_str()))
    })
}

pub(crate) fn evaluate(expression: &str, options: &Options, values: &Values) -> bool {
    let tokens = tokenize(expression);
    let mut parser = Parser {
        tokens: &tokens,
        at: 0,
        options,
        values,
    };
    match parser.or() {
        Some(value) if parser.at == tokens.len() => value,
        _ => true,
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Token {
    Name(String),
    Not,
    And,
    Or,
    Open,
    Close,
}

fn tokenize(expression: &str) -> Vec<Token> {
    let mut tokens = Vec::new();
    let mut chars = expression.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            ' ' | '\t' => {}
            '!' => tokens.push(Token::Not),
            '(' => tokens.push(Token::Open),
            ')' => tokens.push(Token::Close),
            '&' => {
                chars.next_if_eq(&'&');
                tokens.push(Token::And);
            }
            '|' => {
                chars.next_if_eq(&'|');
                tokens.push(Token::Or);
            }
            _ => {
                let mut name = String::from(c);
                while let Some(next) =
                    chars.next_if(|n| !matches!(n, ' ' | '\t' | '!' | '(' | ')' | '&' | '|'))
                {
                    name.push(next);
                }
                tokens.push(Token::Name(name));
            }
        }
    }
    tokens
}

struct Parser<'a> {
    tokens: &'a [Token],
    at: usize,
    options: &'a Options,
    values: &'a Values,
}

impl Parser<'_> {
    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.at)
    }

    fn or(&mut self) -> Option<bool> {
        let mut value = self.and()?;
        while self.peek() == Some(&Token::Or) {
            self.at += 1;
            value |= self.and()?;
        }
        Some(value)
    }

    fn and(&mut self) -> Option<bool> {
        let mut value = self.unary()?;
        while self.peek() == Some(&Token::And) {
            self.at += 1;
            value &= self.unary()?;
        }
        Some(value)
    }

    fn unary(&mut self) -> Option<bool> {
        match self.tokens.get(self.at)? {
            Token::Not => {
                self.at += 1;
                Some(!self.unary()?)
            }
            Token::Open => {
                self.at += 1;
                let value = self.or()?;
                (self.peek() == Some(&Token::Close)).then(|| {
                    self.at += 1;
                    value
                })
            }
            Token::Name(name) => {
                self.at += 1;
                Some(match name.as_str() {
                    "true" | "1" => true,
                    "false" | "0" => false,
                    option => self.options.get(option).is_some_and(|opt| {
                        matches!(opt.kind, Kind::Boolean { .. }) && self.values.get(opt) == "true"
                    }),
                })
            }
            Token::And | Token::Or | Token::Close => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shaderpack::options;

    fn pack() -> (Options, Values) {
        let options = options::discover(
            ["#ifdef ON\n#endif\n#ifdef OFF\n#endif\n#define ON\n//#define OFF\n#define V 1 //[1 2]"].into_iter(),
        );
        (options, Values::default())
    }

    #[test]
    fn expressions_read_the_options() {
        let (options, values) = pack();
        let eval = |e: &str| evaluate(e, &options, &values);
        assert!(eval("ON"));
        assert!(!eval("OFF"));
        assert!(eval("!OFF"));
        assert!(eval("ON && !OFF"));
        assert!(!eval("ON && OFF"));
        assert!(eval("OFF || ON"));
        assert!(eval("(OFF || ON) && ON"));
        assert!(!eval("OFF || ON && OFF"), "&& binds tighter than ||");
        assert!(!eval("V"), "a value option is not a boolean");
        assert!(!eval("NOT_DECLARED"));
        assert!(eval("1") && !eval("false"));
        assert!(eval("ON &&"));
        assert!(eval("(ON"));
    }

    #[test]
    fn a_geometry_falls_back_along_its_chain() {
        let has = |names: &'static [&'static str]| move |n: &str| names.contains(&n);
        assert_eq!(
            resolve(Geometry::Water, has(&["gbuffers_terrain"])),
            Some("gbuffers_terrain")
        );
        assert_eq!(
            resolve(
                Geometry::Water,
                has(&["gbuffers_water", "gbuffers_terrain"])
            ),
            Some("gbuffers_water")
        );
        assert_eq!(
            resolve(Geometry::TerrainCutout, has(&["gbuffers_textured"])),
            Some("gbuffers_textured")
        );
        assert_eq!(resolve(Geometry::TerrainSolid, has(&["composite"])), None);
    }

    #[test]
    fn enable_keys_are_read_and_applied() {
        let (options, values) = pack();
        let keys: std::collections::HashMap<String, String> = [
            ("program.composite3.enabled".to_owned(), "OFF".to_owned()),
            (
                "program.world-1/composite.enabled".to_owned(),
                "ON".to_owned(),
            ),
            (
                "program.world0/deferred..enabled".to_owned(),
                "OFF".to_owned(),
            ),
            ("sun".to_owned(), "true".to_owned()),
        ]
        .into_iter()
        .collect();
        let expressions: Vec<(&str, &str)> = enable_expressions(keys.iter()).collect();
        assert!(!enabled(
            "composite3",
            expressions.iter().copied(),
            &options,
            &values
        ));
        assert!(enabled(
            "composite",
            expressions.iter().copied(),
            &options,
            &values
        ));
        assert!(enabled(
            "world-1/composite",
            expressions.iter().copied(),
            &options,
            &values
        ));
        assert!(!enabled(
            "world0/deferred",
            expressions.iter().copied(),
            &options,
            &values
        ));
    }
}
