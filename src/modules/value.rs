use super::registry::Kind;

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Value {
    Bool(bool),
    Num(f32),
    Range(f32, f32),
    Choice(u8),
}

impl Value {
    pub fn default_of(kind: Kind) -> Value {
        match kind {
            Kind::Toggle { on } => Value::Bool(on),
            Kind::Slider { value, .. } => Value::Num(value),
            Kind::Range { low, high, .. } => Value::Range(low, high),
            Kind::Enum { index, .. } => Value::Choice(index),
            Kind::List { .. } | Kind::Text { .. } => Value::Bool(false),
        }
    }

    pub fn pack(self) -> u64 {
        match self {
            Value::Bool(b) => b as u64,
            Value::Num(x) => x.to_bits() as u64,
            Value::Choice(c) => c as u64,
            Value::Range(lo, hi) => (lo.to_bits() as u64) << 32 | hi.to_bits() as u64,
        }
    }

    pub fn unpack(bits: u64, kind: Kind) -> Value {
        match kind {
            Kind::Toggle { .. } => Value::Bool(bits & 1 != 0),
            Kind::Slider { .. } => Value::Num(f32::from_bits(bits as u32)),
            Kind::Range { .. } => Value::Range(
                f32::from_bits((bits >> 32) as u32),
                f32::from_bits(bits as u32),
            ),
            Kind::Enum { .. } => Value::Choice(bits as u8),
            Kind::List { .. } | Kind::Text { .. } => Value::Bool(false),
        }
    }
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
            assert_eq!(Value::unpack(v.pack(), k), v, "{v:?}");
        }
    }
}
