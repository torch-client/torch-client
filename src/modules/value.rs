use super::registry::Kind;

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Value {
    Bool(bool),
    Num(f32),
    Range(f32, f32),
    Choice(u8),
}

impl Value {
    pub fn default_of(kind: Kind) -> Option<Value> {
        Some(match kind {
            Kind::Toggle { on } => Value::Bool(on),
            Kind::Slider { value, .. } => Value::Num(value),
            Kind::Range { low, high, .. } => Value::Range(low, high),
            Kind::Enum { index, .. } => Value::Choice(index),
            Kind::List { .. } | Kind::Text { .. } => return None,
        })
    }

    pub fn pack(self) -> u64 {
        match self {
            Value::Bool(b) => b as u64,
            Value::Num(x) => x.to_bits() as u64,
            Value::Choice(c) => c as u64,
            Value::Range(lo, hi) => (lo.to_bits() as u64) << 32 | hi.to_bits() as u64,
        }
    }

    pub fn unpack(bits: u64, kind: Kind) -> Option<Value> {
        Some(match kind {
            Kind::Toggle { .. } => Value::Bool(flag(bits)),
            Kind::Slider { .. } => Value::Num(num(bits)),
            Kind::Range { .. } => {
                let (lo, hi) = range(bits);
                Value::Range(lo, hi)
            }
            Kind::Enum { .. } => Value::Choice(choice(bits)),
            Kind::List { .. } | Kind::Text { .. } => return None,
        })
    }
}

pub(super) fn flag(bits: u64) -> bool {
    bits & 1 != 0
}

pub(super) fn num(bits: u64) -> f32 {
    f32::from_bits(bits as u32)
}

pub(super) fn range(bits: u64) -> (f32, f32) {
    (
        f32::from_bits((bits >> 32) as u32),
        f32::from_bits(bits as u32),
    )
}

pub(super) fn choice(bits: u64) -> u8 {
    bits as u8
}

pub(super) fn default_bits(kind: Kind) -> u64 {
    Value::default_of(kind).map_or(0, Value::pack)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn values_round_trip() {
        let cases = [
            (Value::Bool(true), Kind::Toggle { on: false }),
            (
                Value::Num(13.5),
                Kind::Slider {
                    min: 0.0,
                    max: 1.0,
                    value: 0.0,
                    decimals: 0,
                },
            ),
            (
                Value::Choice(3),
                Kind::Enum {
                    options: &[],
                    index: 0,
                },
            ),
            (
                Value::Range(6.0, 13.5),
                Kind::Range {
                    min: 0.0,
                    max: 1.0,
                    low: 0.0,
                    high: 0.0,
                    decimals: 0,
                },
            ),
        ];
        for (v, k) in cases {
            assert_eq!(Value::unpack(v.pack(), k), Some(v), "{v:?}");
        }
    }
}
