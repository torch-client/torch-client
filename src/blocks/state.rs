use std::collections::HashMap;

use crate::items::json::Json;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Rot {
    pub x: u8,
    pub y: u8,
    pub z: u8,
    pub uvlock: bool,
}

impl Rot {
    pub fn is_identity(&self) -> bool {
        self.x == 0 && self.y == 0 && self.z == 0
    }
}

#[derive(Clone, Debug)]
pub struct ModelRef {
    pub model: String,
    pub rot: Rot,
    pub weight: u32,
}

#[derive(Clone, Debug)]
pub struct Slot {
    pub choices: Vec<ModelRef>,
    pub total: u32,
}

impl Slot {
    fn parse(node: &Json) -> Option<Slot> {
        let mut choices = Vec::new();
        match node {
            Json::Arr(items) => {
                for item in items {
                    if let Some(r) = parse_ref(item) {
                        choices.push(r);
                    }
                }
            }
            _ => choices.push(parse_ref(node)?),
        }
        if choices.is_empty() {
            return None;
        }
        let total = choices.iter().map(|c| c.weight).sum();
        Some(Slot { choices, total })
    }
}

fn parse_ref(node: &Json) -> Option<ModelRef> {
    let model = node.get("model").and_then(Json::as_str)?;
    let quadrant = |key: &str| -> u8 {
        let deg = node.get(key).and_then(Json::as_i32).unwrap_or(0);
        (deg.div_euclid(90).rem_euclid(4)) as u8
    };
    Some(ModelRef {
        model: crate::items::model::strip_namespace(model).to_string(),
        rot: Rot {
            x: quadrant("x"),
            y: quadrant("y"),
            z: quadrant("z"),
            uvlock: node.get("uvlock").and_then(Json::as_bool).unwrap_or(false),
        },
        weight: node
            .get("weight")
            .and_then(Json::as_i32)
            .unwrap_or(1)
            .max(0) as u32,
    })
}

#[derive(Clone, Debug)]
pub struct Term {
    value: String,
    negated: bool,
}

impl Term {
    fn parse(text: &str) -> Term {
        match text.strip_prefix('!') {
            Some(rest) => Term {
                value: rest.to_string(),
                negated: true,
            },
            None => Term {
                value: text.to_string(),
                negated: false,
            },
        }
    }

    fn matches(&self, actual: &str) -> bool {
        (self.value == actual) != self.negated
    }
}

#[derive(Clone, Debug)]
pub enum Cond {
    Match(Vec<(String, Vec<Term>)>),
    And(Vec<Cond>),
    Or(Vec<Cond>),
}

impl Cond {
    fn parse(node: &Json) -> Option<Cond> {
        for (key, ctor) in [
            ("OR", Cond::Or as fn(Vec<Cond>) -> Cond),
            ("AND", Cond::And as fn(Vec<Cond>) -> Cond),
        ] {
            if let Some(list) = node.get(key) {
                let terms: Vec<Cond> = list.arr().iter().filter_map(Cond::parse).collect();
                return Some(ctor(terms));
            }
        }
        let Json::Obj(fields) = node else { return None };
        let mut tests = Vec::with_capacity(fields.len());
        for (key, value) in fields {
            let text = json_property_value(value)?;
            let terms: Vec<Term> = text.split('|').map(Term::parse).collect();
            tests.push((key.clone(), terms));
        }
        Some(Cond::Match(tests))
    }

    fn passes(&self, props: &HashMap<&str, &str>) -> bool {
        match self {
            Cond::Match(tests) => tests
                .iter()
                .all(|(key, terms)| match props.get(key.as_str()) {
                    Some(actual) => terms.iter().any(|t| t.matches(actual)),
                    None => false,
                }),
            Cond::And(terms) => terms.iter().all(|c| c.passes(props)),
            Cond::Or(terms) => terms.iter().any(|c| c.passes(props)),
        }
    }
}

fn json_property_value(node: &Json) -> Option<String> {
    match node {
        Json::Str(s) => Some(s.clone()),
        Json::Bool(b) => Some(b.to_string()),
        Json::Num(n) => Some(format!("{}", *n as i64)),
        _ => None,
    }
}

#[derive(Debug)]
pub enum StateDef {
    Variants(Vec<(Vec<(String, String)>, Slot)>),
    Multipart(Vec<(Option<Cond>, Slot)>),
}

impl StateDef {
    pub fn parse(root: &Json) -> Option<StateDef> {
        if let Some(Json::Obj(fields)) = root.get("variants") {
            let mut out = Vec::with_capacity(fields.len());
            for (key, value) in fields {
                let Some(slot) = Slot::parse(value) else {
                    continue;
                };
                out.push((parse_variant_key(key), slot));
            }
            if out.is_empty() {
                return None;
            }
            return Some(StateDef::Variants(out));
        }
        if let Some(parts) = root.get("multipart") {
            let mut out = Vec::new();
            for part in parts.arr() {
                let Some(slot) = part.get("apply").and_then(Slot::parse) else {
                    continue;
                };
                let cond = part.get("when").and_then(Cond::parse);
                out.push((cond, slot));
            }
            if out.is_empty() {
                return None;
            }
            return Some(StateDef::Multipart(out));
        }
        None
    }

    pub fn select(&self, props: &HashMap<&str, &str>) -> Vec<&Slot> {
        match self {
            StateDef::Variants(entries) => entries
                .iter()
                .find(|(key, _)| {
                    key.iter()
                        .all(|(k, v)| props.get(k.as_str()) == Some(&v.as_str()))
                })
                .map(|(_, slot)| vec![slot])
                .unwrap_or_default(),
            StateDef::Multipart(parts) => parts
                .iter()
                .filter(|(cond, _)| cond.as_ref().is_none_or(|c| c.passes(props)))
                .map(|(_, slot)| slot)
                .collect(),
        }
    }
}

fn parse_variant_key(key: &str) -> Vec<(String, String)> {
    key.split(',')
        .filter(|part| !part.is_empty())
        .filter_map(|part| {
            let (k, v) = part.split_once('=')?;
            Some((k.to_string(), v.to_string()))
        })
        .collect()
}
