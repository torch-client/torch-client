use super::{BinaryOp, CustomState, Expr, Function, Unary};

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct Smooth {
    value: f32,
    started: bool,
}

impl Smooth {
    pub(crate) fn update(&mut self, target: f32, up: f32, down: f32, delta: f32) -> f32 {
        if !self.started {
            self.started = true;
            self.value = target;
            return target;
        }
        let half_life = if target > self.value { up } else { down } * 0.1;
        if half_life == 0.0 {
            self.value = target;
            return target;
        }
        let decay = std::f32::consts::LN_2 / half_life;
        let factor = 1.0 - (-decay * delta).exp();
        self.value += (target - self.value) * factor;
        self.value
    }
}

fn truth(v: f32) -> bool {
    v != 0.0
}

pub(super) fn eval(
    expr: &Expr,
    state: &mut CustomState,
    delta: f32,
    input: &dyn Fn(u32) -> f32,
) -> f32 {
    use Function as F;
    let e = |x: &Expr, state: &mut CustomState| eval(x, state, delta, input);
    match expr {
        Expr::Constant(v) => *v,
        Expr::Input(i) => input(*i),
        Expr::Custom(i) => state.values[*i],
        Expr::Unary(Unary::Negate, x) => -e(x, state),
        Expr::Unary(Unary::Not, x) => (!truth(e(x, state))) as u8 as f32,
        Expr::Binary(op, a, b) => {
            let left = e(a, state);
            match op {
                BinaryOp::Or if truth(left) => return 1.0,
                BinaryOp::And if !truth(left) => return 0.0,
                _ => {}
            }
            let right = e(b, state);
            match op {
                BinaryOp::Or | BinaryOp::And => truth(right) as u8 as f32,
                BinaryOp::Equal => (left == right) as u8 as f32,
                BinaryOp::NotEqual => (left != right) as u8 as f32,
                BinaryOp::Less => (left < right) as u8 as f32,
                BinaryOp::Greater => (left > right) as u8 as f32,
                BinaryOp::LessEqual => (left <= right) as u8 as f32,
                BinaryOp::GreaterEqual => (left >= right) as u8 as f32,
                BinaryOp::Add => left + right,
                BinaryOp::Subtract => left - right,
                BinaryOp::Multiply => left * right,
                BinaryOp::Divide => left / right,
                BinaryOp::Remainder => left % right,
            }
        }
        Expr::Call(function, args) => {
            let arg = |i: usize, state: &mut CustomState| e(&args[i], state);
            match function {
                F::Sin => arg(0, state).sin(),
                F::Cos => arg(0, state).cos(),
                F::Tan => arg(0, state).tan(),
                F::Asin => arg(0, state).asin(),
                F::Acos => arg(0, state).acos(),
                F::Atan => arg(0, state).atan(),
                F::Atan2 => {
                    let y = arg(0, state);
                    y.atan2(arg(1, state))
                }
                F::Radians => arg(0, state).to_radians(),
                F::Degrees => arg(0, state).to_degrees(),
                F::Min => {
                    let a = arg(0, state);
                    a.min(arg(1, state))
                }
                F::Max => {
                    let a = arg(0, state);
                    a.max(arg(1, state))
                }
                F::Clamp => {
                    let (x, lo) = (arg(0, state), arg(1, state));
                    x.max(lo).min(arg(2, state))
                }
                F::Abs => arg(0, state).abs(),
                F::Floor => arg(0, state).floor(),
                F::Ceil => arg(0, state).ceil(),
                F::Exp => arg(0, state).exp(),
                F::Exp2 => arg(0, state).exp2(),
                F::Exp10 => 10f32.powf(arg(0, state)),
                F::Log => arg(0, state).ln(),
                F::Log2 => arg(0, state).log2(),
                F::Log10 => arg(0, state).log10(),
                F::Frac => {
                    let x = arg(0, state);
                    x - x.floor()
                }
                F::Pow => {
                    let b = arg(0, state);
                    b.powf(arg(1, state))
                }
                F::Sqrt => arg(0, state).sqrt(),
                F::Sign => {
                    let x = arg(0, state);
                    if x == 0.0 { 0.0 } else { x.signum() }
                }
                F::Fmod => {
                    let a = arg(0, state);
                    a % arg(1, state)
                }
                F::Random => {
                    state.random ^= state.random << 13;
                    state.random ^= state.random >> 17;
                    state.random ^= state.random << 5;
                    (state.random >> 8) as f32 / (1u32 << 24) as f32
                }
                F::Between => {
                    let (x, lo) = (arg(0, state), arg(1, state));
                    (x >= lo && x <= arg(2, state)) as u8 as f32
                }
                F::Equals => {
                    let (a, b) = (arg(0, state), arg(1, state));
                    ((a - b).abs() <= arg(2, state)) as u8 as f32
                }
            }
        }
        Expr::If(args) => {
            let mut i = 0;
            while i + 1 < args.len() {
                if truth(e(&args[i], state)) {
                    return e(&args[i + 1], state);
                }
                i += 2;
            }
            args.last().map_or(0.0, |otherwise| e(otherwise, state))
        }
        Expr::In(x, rest) => {
            let x = e(x, state);
            rest.iter().any(|r| e(r, state) == x) as u8 as f32
        }
        Expr::Smooth {
            slot,
            value,
            up,
            down,
        } => {
            let (target, up, down) = (e(value, state), e(up, state), e(down, state));
            state.smooth[*slot].update(target, up, down, delta)
        }
    }
}
