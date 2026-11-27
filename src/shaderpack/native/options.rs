use std::collections::HashMap;

use super::caps::TierName;
use super::problem::Position;

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Kind {
    Bool,
    Int { min: i32, max: i32 },
    Float { min: f64, max: f64, step: f64 },
    Choice { values: Vec<String> },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum Field {
    Min,
    Max,
    Step,
    Values,
}

pub(crate) const CHOICE_VALUES: std::ops::RangeInclusive<usize> = 2..=32;

impl Kind {
    pub(crate) fn type_name(&self) -> &'static str {
        match self {
            Kind::Bool => "bool",
            Kind::Int { .. } => "int",
            Kind::Float { .. } => "float",
            Kind::Choice { .. } => "choice",
        }
    }

    pub(crate) fn wgsl_type(&self) -> &'static str {
        match self {
            Kind::Bool => "bool",
            Kind::Int { .. } => "i32",
            Kind::Float { .. } => "f32",
            Kind::Choice { .. } => "u32",
        }
    }

    pub(crate) fn problems(&self) -> Vec<(Field, String)> {
        let mut out = Vec::new();
        match self {
            Kind::Bool => {}
            Kind::Int { min, max } => {
                if min >= max {
                    out.push((
                        Field::Max,
                        format!("max ({max}) must be greater than min ({min})"),
                    ));
                }
            }
            Kind::Float { min, max, step } => {
                if !min.is_finite() {
                    out.push((Field::Min, "min must be a finite number".to_owned()));
                }
                if !max.is_finite() {
                    out.push((Field::Max, "max must be a finite number".to_owned()));
                }
                if min.is_finite() && max.is_finite() && min >= max {
                    out.push((
                        Field::Max,
                        format!("max ({max}) must be greater than min ({min})"),
                    ));
                }
                if !step.is_finite() || *step <= 0.0 {
                    out.push((Field::Step, "step must be a positive number".to_owned()));
                } else if min < max && *step > max - min {
                    out.push((
                        Field::Step,
                        format!("step ({step}) is larger than the range {min}..{max}"),
                    ));
                }
            }
            Kind::Choice { values } => {
                if !CHOICE_VALUES.contains(&values.len()) {
                    out.push((
                        Field::Values,
                        format!(
                            "a choice has {} to {} values, not {}",
                            CHOICE_VALUES.start(),
                            CHOICE_VALUES.end(),
                            values.len()
                        ),
                    ));
                }
                for (i, value) in values.iter().enumerate() {
                    if !is_value_name(value) {
                        out.push((
                            Field::Values,
                            format!("`{value}` is not a value name; use [a-z0-9_]+"),
                        ));
                    } else if values[..i].contains(value) {
                        out.push((Field::Values, format!("`{value}` is listed twice")));
                    }
                }
            }
        }
        out
    }
}

pub(crate) fn is_value_name(value: &str) -> bool {
    !value.is_empty()
        && value
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Value {
    Bool(bool),
    Int(i32),
    Float(f64),
    Choice(u32),
}

impl Value {
    pub(crate) fn override_value(self) -> f64 {
        match self {
            Value::Bool(b) => f64::from(u8::from(b)),
            Value::Int(i) => f64::from(i),
            Value::Float(f) => f,
            Value::Choice(i) => f64::from(i),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Literal<'a> {
    Bool(bool),
    Int(i64),
    Float(f64),
    Str(&'a str),
}

#[derive(Clone, Debug)]
pub(crate) struct Decl {
    pub(crate) id: String,
    pub(crate) kind: Kind,
    pub(crate) default: Value,
    #[allow(
        dead_code,
        reason = "phase 4 of docs/SHADER_FORMAT.md, the options screen, shows it"
    )]
    pub(crate) name: Option<String>,
    pub(crate) at: Position,
}

impl Kind {
    pub(crate) fn check(&self, value: Value) -> Result<(), String> {
        match (self, value) {
            (Kind::Bool, Value::Bool(_)) => Ok(()),
            (Kind::Int { min, max }, Value::Int(v)) => {
                if (*min..=*max).contains(&v) {
                    Ok(())
                } else {
                    Err(format!("{v} is outside {min}..{max}"))
                }
            }
            (Kind::Float { min, max, .. }, Value::Float(v)) => {
                if !v.is_finite() {
                    Err(format!("{v} is not a finite number"))
                } else if v < *min || v > *max {
                    Err(format!("{v} is outside {min}..{max}"))
                } else {
                    Ok(())
                }
            }
            (Kind::Choice { values }, Value::Choice(i)) => {
                if (i as usize) < values.len() {
                    Ok(())
                } else {
                    Err(format!(
                        "choice index {i} is past the {} values",
                        values.len()
                    ))
                }
            }
            (kind, _) => Err(format!("this is a {} option", kind.type_name())),
        }
    }

    pub(crate) fn accept(&self, literal: Literal<'_>) -> Result<Value, String> {
        let value = match (self, literal) {
            (Kind::Bool, Literal::Bool(b)) => Value::Bool(b),
            (Kind::Int { .. }, Literal::Int(i)) => {
                Value::Int(i32::try_from(i).map_err(|_| format!("{i} does not fit an i32"))?)
            }
            (Kind::Float { .. }, Literal::Float(f)) => Value::Float(f),
            (Kind::Float { .. }, Literal::Int(i)) => Value::Float(i as f64),
            (Kind::Choice { values }, Literal::Str(s)) => {
                match values.iter().position(|v| v == s) {
                    Some(i) => Value::Choice(i as u32),
                    None => return Err(format!("`{s}` is not one of {}", values.join(", "))),
                }
            }
            (kind, _) => {
                return Err(format!(
                    "a {} option takes {}",
                    kind.type_name(),
                    match kind {
                        Kind::Bool => "true or false",
                        Kind::Int { .. } => "an integer",
                        Kind::Float { .. } => "a number",
                        Kind::Choice { .. } => "one of its values as a string",
                    }
                ));
            }
        };
        self.check(value)?;
        Ok(value)
    }

    #[allow(
        dead_code,
        reason = "phase 4 of docs/SHADER_FORMAT.md reads the saved option file"
    )]
    pub(crate) fn parse(&self, text: &str) -> Option<Value> {
        let value = match self {
            Kind::Bool => Value::Bool(text.parse().ok()?),
            Kind::Int { .. } => Value::Int(text.parse().ok()?),
            Kind::Float { .. } => Value::Float(text.parse().ok()?),
            Kind::Choice { values } => Value::Choice(values.iter().position(|v| v == text)? as u32),
        };
        self.check(value).ok().map(|()| value)
    }

    #[allow(
        dead_code,
        reason = "phase 4 of docs/SHADER_FORMAT.md writes the saved option file"
    )]
    pub(crate) fn text(&self, value: Value) -> String {
        match (value, self) {
            (Value::Choice(i), Kind::Choice { values }) => {
                values.get(i as usize).cloned().unwrap_or_default()
            }
            (Value::Bool(b), _) => b.to_string(),
            (Value::Int(i), _) => i.to_string(),
            (Value::Float(f), _) => f.to_string(),
            (Value::Choice(i), _) => i.to_string(),
        }
    }
}

pub(crate) fn constant_name(option: &str, value: &str) -> String {
    format!(
        "{}_{}",
        option.to_ascii_uppercase(),
        value.to_ascii_uppercase()
    )
}

#[derive(Clone, Debug, Default)]
pub(crate) struct Options {
    all: Vec<Decl>,
    index: HashMap<String, usize>,
}

impl Options {
    pub(crate) fn new(decls: Vec<Decl>) -> Options {
        let mut options = Options::default();
        for decl in decls {
            if !options.index.contains_key(&decl.id) {
                options.index.insert(decl.id.clone(), options.all.len());
                options.all.push(decl);
            }
        }
        options
    }

    pub(crate) fn get(&self, id: &str) -> Option<&Decl> {
        self.index.get(id).map(|&i| &self.all[i])
    }

    pub(crate) fn iter(&self) -> std::slice::Iter<'_, Decl> {
        self.all.iter()
    }

    pub(crate) fn overrides<'a>(&'a self, values: &Values) -> Vec<(&'a str, f64)> {
        self.all
            .iter()
            .map(|decl| {
                (
                    decl.id.as_str(),
                    values
                        .get(&decl.id)
                        .unwrap_or(decl.default)
                        .override_value(),
                )
            })
            .collect()
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Values {
    current: HashMap<String, Value>,
}

impl Values {
    pub(crate) fn defaults(options: &Options) -> Values {
        Values {
            current: options
                .iter()
                .map(|decl| (decl.id.clone(), decl.default))
                .collect(),
        }
    }

    pub(crate) fn load(text: &str, options: &Options) -> Values {
        let mut values = Values::defaults(options);
        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let Some((id, text)) = line.split_once('=') else {
                continue;
            };
            if let Some(decl) = options.get(id.trim())
                && let Some(value) = decl.kind.parse(text.trim())
            {
                values.current.insert(decl.id.clone(), value);
            }
        }
        values
    }

    #[allow(
        dead_code,
        reason = "phase 4 of docs/SHADER_FORMAT.md writes the saved option file"
    )]
    pub(crate) fn save(&self, options: &Options) -> String {
        let mut lines: Vec<String> = options
            .iter()
            .filter_map(|decl| {
                let value = self.get(&decl.id)?;
                (value != decl.default).then(|| format!("{}={}", decl.id, decl.kind.text(value)))
            })
            .collect();
        lines.sort();
        lines.join("\n")
    }

    pub(crate) fn get(&self, id: &str) -> Option<Value> {
        self.current.get(id).copied()
    }

    #[allow(
        dead_code,
        reason = "phase 4 of docs/SHADER_FORMAT.md, the options screen, sets options"
    )]
    pub(crate) fn set(&mut self, options: &Options, id: &str, value: Value) -> Result<(), String> {
        let decl = options.get(id).ok_or_else(|| format!("no option `{id}`"))?;
        decl.kind.check(value)?;
        self.current.insert(decl.id.clone(), value);
        Ok(())
    }

    pub(crate) fn apply(&mut self, preset: &Preset) {
        for (id, value) in &preset.set {
            self.current.insert(id.clone(), *value);
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct Preset {
    pub(crate) id: String,
    #[allow(
        dead_code,
        reason = "phase 4 of docs/SHADER_FORMAT.md, the options screen, shows it"
    )]
    pub(crate) name: Option<String>,
    pub(crate) set: Vec<(String, Value)>,
    pub(crate) default_on: Vec<(TierName, Position)>,
    pub(crate) at: Position,
}

impl Preset {
    #[allow(
        dead_code,
        reason = "phase 4 of docs/SHADER_FORMAT.md, the options screen's preset cycler"
    )]
    pub(crate) fn matches(&self, values: &Values) -> bool {
        self.set
            .iter()
            .all(|(id, value)| values.get(id) == Some(*value))
    }
}

#[allow(
    dead_code,
    reason = "phase 4 of docs/SHADER_FORMAT.md, the options screen's preset cycler"
)]
pub(crate) fn current_preset<'a>(presets: &'a [Preset], values: &Values) -> Option<&'a Preset> {
    presets.iter().find(|preset| preset.matches(values))
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    pub(crate) fn decl(id: &str, kind: Kind, default: Value) -> Decl {
        Decl {
            id: id.to_owned(),
            kind,
            default,
            name: None,
            at: Position { line: 1, column: 1 },
        }
    }

    pub(crate) fn sample() -> Options {
        Options::new(vec![
            decl("shadows", Kind::Bool, Value::Bool(true)),
            decl("samples", Kind::Int { min: 1, max: 16 }, Value::Int(4)),
            decl(
                "exposure",
                Kind::Float {
                    min: 0.25,
                    max: 4.0,
                    step: 0.05,
                },
                Value::Float(1.0),
            ),
            decl(
                "quality",
                Kind::Choice {
                    values: vec!["low".into(), "medium".into(), "high".into()],
                },
                Value::Choice(1),
            ),
        ])
    }

    #[test]
    fn only_changed_values_are_saved_and_they_read_back() {
        let options = sample();
        let mut values = Values::defaults(&options);
        assert_eq!(values.save(&options), "");
        values.set(&options, "exposure", Value::Float(2.5)).unwrap();
        values.set(&options, "quality", Value::Choice(2)).unwrap();
        values.set(&options, "shadows", Value::Bool(false)).unwrap();
        let text = values.save(&options);
        assert_eq!(text, "exposure=2.5\nquality=high\nshadows=false");
        assert_eq!(Values::load(&text, &options), values);
    }

    #[test]
    fn a_choice_survives_reordering_its_values() {
        let before = sample();
        let mut values = Values::defaults(&before);
        values.set(&before, "quality", Value::Choice(2)).unwrap();
        let saved = values.save(&before);

        let after = Options::new(vec![decl(
            "quality",
            Kind::Choice {
                values: vec!["ultra".into(), "high".into(), "low".into(), "medium".into()],
            },
            Value::Choice(3),
        )]);
        let loaded = Values::load(&saved, &after);
        assert_eq!(loaded.get("quality"), Some(Value::Choice(1)));
    }

    #[test]
    fn bad_saved_values_are_dropped() {
        let options = sample();
        let text = "exposure=NaN\nsamples=99\nquality=gone\nshadows=maybe\nunknown=1\nnot a line\n";
        assert_eq!(Values::load(text, &options), Values::defaults(&options));
        let text = "exposure=inf\nexposure=-inf\nsamples=2.5\n";
        assert_eq!(Values::load(text, &options), Values::defaults(&options));
    }

    #[test]
    fn manifest_literals_are_typed_and_ranged() {
        let options = sample();
        let exposure = &options.get("exposure").unwrap().kind;
        assert_eq!(exposure.accept(Literal::Int(2)), Ok(Value::Float(2.0)));
        assert!(exposure.accept(Literal::Float(5.0)).is_err());
        assert!(exposure.accept(Literal::Float(f64::NAN)).is_err());
        assert!(exposure.accept(Literal::Bool(true)).is_err());
        let samples = &options.get("samples").unwrap().kind;
        assert!(samples.accept(Literal::Float(2.0)).is_err());
        assert!(samples.accept(Literal::Int(i64::MAX)).is_err());
        let quality = &options.get("quality").unwrap().kind;
        assert_eq!(quality.accept(Literal::Str("high")), Ok(Value::Choice(2)));
        assert!(quality.accept(Literal::Str("ultra")).is_err());
    }

    #[test]
    fn declarations_are_checked() {
        assert_eq!(Kind::Int { min: 3, max: 3 }.problems()[0].0, Field::Max);
        let float = Kind::Float {
            min: 0.0,
            max: 1.0,
            step: 2.0,
        };
        assert_eq!(float.problems()[0].0, Field::Step);
        let one = Kind::Choice {
            values: vec!["a".into()],
        };
        assert_eq!(one.problems().len(), 1);
        let bad = Kind::Choice {
            values: vec!["a".into(), "B".into(), "a".into()],
        };
        assert_eq!(bad.problems().len(), 2);
    }

    #[test]
    fn the_wgsl_side_gets_types_values_and_constants() {
        let options = sample();
        let values = Values::defaults(&options);
        assert_eq!(
            options.overrides(&values),
            [
                ("shadows", 1.0),
                ("samples", 4.0),
                ("exposure", 1.0),
                ("quality", 1.0)
            ]
        );
        let quality = options.get("quality").unwrap();
        assert_eq!(quality.kind.wgsl_type(), "u32");
        assert_eq!(constant_name(&quality.id, "medium"), "QUALITY_MEDIUM");
    }

    #[test]
    fn presets_match_only_what_they_name() {
        let options = sample();
        let low = Preset {
            id: "low".into(),
            name: None,
            set: vec![
                ("shadows".into(), Value::Bool(false)),
                ("quality".into(), Value::Choice(0)),
            ],
            default_on: Vec::new(),
            at: Position { line: 1, column: 1 },
        };
        let presets = [low];
        let mut values = Values::defaults(&options);
        assert!(current_preset(&presets, &values).is_none());
        values.apply(&presets[0]);
        values.set(&options, "exposure", Value::Float(3.0)).unwrap();
        assert_eq!(
            current_preset(&presets, &values).map(|p| p.id.as_str()),
            Some("low")
        );
    }
}
