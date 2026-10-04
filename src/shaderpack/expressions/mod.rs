mod eval;
mod parse;

pub(crate) use eval::Smooth;
use eval::eval;
use parse::{Parser, tokenize};

use std::collections::HashMap;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Resolved {
    Input(u32),
    Constant(f32),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CustomType {
    Float,
    Int,
    Bool,
}

impl CustomType {
    fn parse(name: &str) -> Option<CustomType> {
        match name {
            "float" => Some(CustomType::Float),
            "int" => Some(CustomType::Int),
            "bool" => Some(CustomType::Bool),
            _ => None,
        }
    }

    pub(crate) fn glsl(self) -> &'static str {
        match self {
            CustomType::Float => "float",
            CustomType::Int => "int",
            CustomType::Bool => "bool",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Unary {
    Negate,
    Not,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum BinaryOp {
    Or,
    And,
    Equal,
    NotEqual,
    Less,
    Greater,
    LessEqual,
    GreaterEqual,
    Add,
    Subtract,
    Multiply,
    Divide,
    Remainder,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Function {
    Sin,
    Cos,
    Tan,
    Asin,
    Acos,
    Atan,
    Atan2,
    Radians,
    Degrees,
    Min,
    Max,
    Clamp,
    Abs,
    Floor,
    Ceil,
    Exp,
    Exp2,
    Exp10,
    Log,
    Log2,
    Log10,
    Frac,
    Pow,
    Sqrt,
    Sign,
    Fmod,
    Random,
    Between,
    Equals,
}

#[derive(Clone, Debug, PartialEq)]
pub(super) enum Expr {
    Constant(f32),
    Input(u32),
    Custom(usize),
    Unary(Unary, Box<Expr>),
    Binary(BinaryOp, Box<Expr>, Box<Expr>),
    Call(Function, Vec<Expr>),
    If(Vec<Expr>),
    In(Box<Expr>, Vec<Expr>),
    Smooth {
        slot: usize,
        value: Box<Expr>,
        up: Box<Expr>,
        down: Box<Expr>,
    },
}

#[derive(Clone, Debug)]
pub(crate) struct Custom {
    pub(crate) name: String,
    pub(crate) ty: CustomType,
    pub(crate) uniform: bool,
    expr: Expr,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct Customs {
    pub(crate) entries: Vec<Custom>,
    order: Vec<usize>,
    smooth_slots: usize,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct CustomState {
    pub(super) smooth: Vec<Smooth>,
    pub(crate) values: Vec<f32>,
    pub(super) random: u32,
}

impl Customs {
    pub(crate) fn compile<'a>(
        keys: impl Iterator<Item = (&'a String, &'a String)>,
        resolve: &dyn Fn(&str, Option<usize>) -> Option<Resolved>,
    ) -> (Customs, Vec<String>) {
        let mut problems = Vec::new();
        let mut declared: Vec<(String, CustomType, bool, String)> = Vec::new();
        for (key, text) in keys {
            let (uniform, rest) = match key.split_once('.') {
                Some(("uniform", rest)) => (true, rest),
                Some(("variable", rest)) => (false, rest),
                _ => continue,
            };
            let Some((ty, name)) = rest.split_once('.') else {
                continue;
            };
            match CustomType::parse(ty) {
                Some(ty) => declared.push((name.to_owned(), ty, uniform, text.clone())),
                None => problems.push(format!("{key}: {ty} customs are not supported yet")),
            }
        }
        declared.sort_by(|a, b| a.0.cmp(&b.0));
        let index: HashMap<&str, usize> = declared
            .iter()
            .enumerate()
            .map(|(i, d)| (d.0.as_str(), i))
            .collect();

        let mut smooth_slots = 0;
        let mut compiled: Vec<Option<Expr>> = Vec::with_capacity(declared.len());
        for (name, _, _, text) in &declared {
            let mut parser = Parser {
                tokens: tokenize(text),
                at: 0,
                resolve,
                customs: &index,
                smooth_slots: &mut smooth_slots,
            };
            match parser.expression().and_then(|e| {
                if parser.at == parser.tokens.len() {
                    Ok(e)
                } else {
                    Err(format!("unexpected {:?}", parser.tokens[parser.at]))
                }
            }) {
                Ok(expr) => compiled.push(Some(expr)),
                Err(e) => {
                    problems.push(format!("{name}: {e}"));
                    compiled.push(None);
                }
            }
        }

        let mut state = vec![Visit::New; declared.len()];
        let mut order = Vec::with_capacity(declared.len());
        for start in 0..declared.len() {
            visit(start, &compiled, &mut state, &mut order);
        }
        for (i, visit) in state.iter().enumerate() {
            if *visit == Visit::Dropped && compiled[i].is_some() {
                problems.push(format!(
                    "{}: reads a custom that failed or a cycle",
                    declared[i].0
                ));
            }
        }

        let entries = declared
            .into_iter()
            .zip(compiled)
            .map(|((name, ty, uniform, _), expr)| Custom {
                name,
                ty,
                uniform,
                expr: expr.unwrap_or(Expr::Constant(0.0)),
            })
            .collect();
        (
            Customs {
                entries,
                order,
                smooth_slots,
            },
            problems,
        )
    }

    pub(crate) fn state(&self) -> CustomState {
        CustomState {
            smooth: vec![Smooth::default(); self.smooth_slots],
            values: vec![0.0; self.entries.len()],
            random: 0x2545_f491,
        }
    }

    pub(crate) fn evaluate(&self, state: &mut CustomState, delta: f32, input: &dyn Fn(u32) -> f32) {
        for &index in &self.order {
            let value = eval(&self.entries[index].expr, state, delta, input);
            let value = match self.entries[index].ty {
                CustomType::Float => value,
                CustomType::Int => value.trunc(),
                CustomType::Bool => (value != 0.0) as u8 as f32,
            };
            state.values[index] = value;
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Visit {
    New,
    Visiting,
    Done,
    Dropped,
}

fn visit(
    at: usize,
    compiled: &[Option<Expr>],
    state: &mut [Visit],
    order: &mut Vec<usize>,
) -> bool {
    match state[at] {
        Visit::Done => return true,
        Visit::Dropped | Visit::Visiting => {
            state[at] = Visit::Dropped;
            return false;
        }
        Visit::New => {}
    }
    let Some(expr) = &compiled[at] else {
        state[at] = Visit::Dropped;
        return false;
    };
    state[at] = Visit::Visiting;
    let mut reads = Vec::new();
    customs_read(expr, &mut reads);
    for read in reads {
        if !visit(read, compiled, state, order) {
            state[at] = Visit::Dropped;
            return false;
        }
    }
    state[at] = Visit::Done;
    order.push(at);
    true
}

fn customs_read(expr: &Expr, out: &mut Vec<usize>) {
    match expr {
        Expr::Custom(i) => out.push(*i),
        Expr::Constant(_) | Expr::Input(_) => {}
        Expr::Unary(_, e) => customs_read(e, out),
        Expr::Binary(_, a, b) => {
            customs_read(a, out);
            customs_read(b, out);
        }
        Expr::Call(_, args) | Expr::If(args) => args.iter().for_each(|a| customs_read(a, out)),
        Expr::In(x, rest) => {
            customs_read(x, out);
            rest.iter().for_each(|a| customs_read(a, out));
        }
        Expr::Smooth {
            value, up, down, ..
        } => {
            customs_read(value, out);
            customs_read(up, out);
            customs_read(down, out);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn resolve(name: &str, member: Option<usize>) -> Option<Resolved> {
        match (name, member) {
            ("sunAngle", None) => Some(Resolved::Input(0)),
            ("cameraPosition", Some(1)) => Some(Resolved::Input(1)),
            ("biome", None) => Some(Resolved::Input(2)),
            ("BIOME_DESERT", None) => Some(Resolved::Constant(7.0)),
            _ => None,
        }
    }

    fn keys(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
        pairs
            .iter()
            .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
            .collect()
    }

    fn compile(pairs: &[(&str, &str)]) -> (Customs, Vec<String>) {
        let keys = keys(pairs);
        Customs::compile(keys.iter().map(|(k, v)| (k, v)), &resolve)
    }

    fn value(customs: &Customs, state: &CustomState, name: &str) -> f32 {
        let i = customs.entries.iter().position(|c| c.name == name).unwrap();
        state.values[i]
    }

    #[test]
    fn solas_time_of_day_evaluates_in_dependency_order() {
        let (customs, problems) = compile(&[
            (
                "uniform.float.timeBrightness",
                "max(sin(timeAngle * 6.28318530718), 0.0)",
            ),
            (
                "uniform.float.timeAngle",
                "(timeAfrc * (1.0 - timeAmix) + timeAfrs * timeAmix + timeHalf) * 0.5",
            ),
            ("variable.float.timeAmix", "if(timeHalf < 0.5, 0.3, -0.1)"),
            (
                "variable.float.timeAfrs",
                "timeAfrc * timeAfrc * (3.0 - 2.0 * timeAfrc)",
            ),
            ("variable.float.timeAfrc", "frac(timeAlin * 2.0)"),
            ("variable.float.timeHalf", "if(timeAlin > 0.5, 1.0, 0.0)"),
            (
                "variable.float.timeAlin",
                "if(timeAmin < 0.433333333, timeAmin * 1.15384615385, timeAmin * 0.882352941176 + 0.117647058824)",
            ),
            ("variable.float.timeAmin", "frac(sunAngle - 0.033333333)"),
        ]);
        assert!(problems.is_empty(), "{problems:?}");
        let mut state = customs.state();
        customs.evaluate(&mut state, 0.016, &|i| if i == 0 { 0.25 } else { 0.0 });
        let angle = value(&customs, &state, "timeAngle");
        assert!((angle - 0.25).abs() < 0.02, "timeAngle at noon is {angle}");
        assert!(value(&customs, &state, "timeBrightness") > 0.99);
    }

    #[test]
    fn biomes_members_and_integers_evaluate() {
        let (customs, problems) = compile(&[
            (
                "uniform.float.isDesert",
                "if(in(biome, BIOME_DESERT), 1, 0)",
            ),
            ("variable.float.yCold", "if(cameraPosition.y >= 93.0, 1, 0)"),
            ("uniform.int.framemod8", "13 % 8"),
            ("uniform.bool.both", "isDesert > 0.5 && yCold > 0.5"),
        ]);
        assert!(problems.is_empty(), "{problems:?}");
        let mut state = customs.state();
        customs.evaluate(&mut state, 0.016, &|i| match i {
            1 => 100.0,
            2 => 7.0,
            _ => 0.0,
        });
        assert_eq!(value(&customs, &state, "isDesert"), 1.0);
        assert_eq!(value(&customs, &state, "yCold"), 1.0);
        assert_eq!(value(&customs, &state, "framemod8"), 5.0);
        assert_eq!(value(&customs, &state, "both"), 1.0);
    }

    #[test]
    fn smooth_starts_at_its_target_and_decays_by_half_life() {
        let (customs, problems) = compile(&[("uniform.float.s", "smooth(1, biome, 10, 10)")]);
        assert!(problems.is_empty(), "{problems:?}");
        let mut state = customs.state();
        customs.evaluate(&mut state, 0.0, &|_| 0.0);
        assert_eq!(value(&customs, &state, "s"), 0.0);
        customs.evaluate(&mut state, 1.0, &|_| 1.0);
        assert!((value(&customs, &state, "s") - 0.5).abs() < 1e-4);
    }

    #[test]
    fn exponents_are_read_as_part_of_a_number() {
        let (customs, problems) = compile(&[("uniform.float.small", "1e-3 * 2.5E2 + 2e1f")]);
        assert!(problems.is_empty(), "{problems:?}");
        let mut state = customs.state();
        customs.evaluate(&mut state, 0.0, &|_| 0.0);
        assert!((value(&customs, &state, "small") - 20.25).abs() < 1e-5);
    }

    #[test]
    fn failures_are_reported_and_their_readers_dropped() {
        let (customs, problems) = compile(&[
            ("uniform.float.a", "notAUniform + 1"),
            ("uniform.float.b", "a * 2"),
            ("uniform.float.c", "d"),
            ("uniform.float.d", "c"),
            ("uniform.vec3.v", "vec3(1, 2, 3)"),
            ("uniform.float.ok", "sunAngle"),
        ]);
        assert_eq!(problems.len(), 5, "{problems:?}");
        let mut state = customs.state();
        customs.evaluate(&mut state, 0.0, &|_| 0.5);
        assert_eq!(value(&customs, &state, "ok"), 0.5);
        assert_eq!(value(&customs, &state, "b"), 0.0);
    }
}
