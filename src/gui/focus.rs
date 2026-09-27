use std::cell::Cell;

struct Ring {
    current: Cell<i32>,
    seq: Cell<i32>,
    step: Cell<i32>,
    first: Cell<i32>,
    last: Cell<i32>,
    prev: Cell<i32>,
    next: Cell<i32>,
    claim: Cell<i32>,
    enabled: Cell<bool>,
    depth: Cell<u32>,
}

impl Ring {
    const fn new() -> Ring {
        Ring {
            current: Cell::new(-1),
            seq: Cell::new(0),
            step: Cell::new(0),
            first: Cell::new(-1),
            last: Cell::new(-1),
            prev: Cell::new(-1),
            next: Cell::new(-1),
            claim: Cell::new(-1),
            enabled: Cell::new(false),
            depth: Cell::new(0),
        }
    }
}

thread_local! {
    static RING: Ring = const { Ring::new() };
}

pub fn begin(enabled: bool, step: i32) {
    RING.with(|r| {
        r.enabled.set(enabled);
        r.step.set(if enabled { step } else { 0 });
        r.seq.set(0);
        r.first.set(-1);
        r.last.set(-1);
        r.prev.set(-1);
        r.next.set(-1);
        r.claim.set(-1);
        r.depth.set(0);
        if !enabled {
            r.current.set(-1);
        }
    });
}

pub fn next(active: bool) -> bool {
    RING.with(|r| {
        if !r.enabled.get() || r.depth.get() > 0 {
            return false;
        }
        let i = r.seq.get();
        r.seq.set(i + 1);
        if !active {
            return false;
        }
        let c = r.current.get();
        if r.first.get() < 0 {
            r.first.set(i);
        }
        r.last.set(i);
        if i < c {
            r.prev.set(i);
        } else if i > c && r.next.get() < 0 {
            r.next.set(i);
        }
        i == c
    })
}

pub fn claim() {
    RING.with(|r| {
        if !r.enabled.get() || r.depth.get() > 0 {
            return;
        }
        let i = r.seq.get() - 1;
        if i >= 0 {
            r.claim.set(i);
        }
    });
}

pub fn enabled() -> bool {
    RING.with(|r| r.enabled.get())
}

pub fn suspend() {
    RING.with(|r| r.depth.set(r.depth.get() + 1));
}

pub fn resume() {
    RING.with(|r| r.depth.set(r.depth.get().saturating_sub(1)));
}

pub fn end() {
    RING.with(|r| {
        if !r.enabled.get() {
            return;
        }
        let claimed = r.claim.get();
        if claimed >= 0 {
            r.current.set(claimed);
        } else {
            let step = r.step.get();
            if step > 0 {
                let n = r.next.get();
                r.current.set(if n >= 0 { n } else { r.first.get() });
            } else if step < 0 {
                let p = r.prev.get();
                r.current.set(if p >= 0 { p } else { r.last.get() });
            }
        }
        if r.current.get() >= r.seq.get() {
            r.current.set(-1);
        }
        r.step.set(0);
    });
}

pub fn clear() {
    RING.with(|r| r.current.set(-1));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(step: i32, count: i32, inactive: &[i32]) -> i32 {
        begin(true, step);
        for i in 0..count {
            next(!inactive.contains(&i));
        }
        end();
        RING.with(|r| r.current.get())
    }

    #[test]
    fn the_first_step_picks_an_end() {
        clear();
        assert_eq!(frame(1, 4, &[]), 0);
        clear();
        assert_eq!(frame(-1, 4, &[]), 3);
    }

    #[test]
    fn stepping_off_an_end_wraps() {
        clear();
        for expected in [0, 1, 2, 0, 1] {
            assert_eq!(frame(1, 3, &[]), expected);
        }
        clear();
        for expected in [2, 1, 0, 2] {
            assert_eq!(frame(-1, 3, &[]), expected);
        }
    }

    #[test]
    fn inactive_widgets_are_skipped_but_keep_their_index() {
        clear();
        assert_eq!(frame(1, 4, &[1, 2]), 0);
        assert_eq!(frame(1, 4, &[1, 2]), 3);
        assert_eq!(frame(-1, 4, &[1, 2]), 0);
    }

    #[test]
    fn a_screen_with_no_active_widgets_stays_unfocused() {
        clear();
        assert_eq!(frame(1, 3, &[0, 1, 2]), -1);
        assert_eq!(frame(-1, 3, &[0, 1, 2]), -1);
    }

    #[test]
    fn suspended_widgets_take_no_index() {
        clear();
        begin(true, 1);
        next(true);
        suspend();
        for _ in 0..20 {
            next(true);
        }
        resume();
        next(true);
        end();
        assert_eq!(RING.with(|r| r.current.get()), 0);
        begin(true, 1);
        next(true);
        suspend();
        next(true);
        resume();
        next(true);
        end();
        assert_eq!(RING.with(|r| r.current.get()), 1);
    }

    #[test]
    fn a_click_takes_the_focus() {
        clear();
        begin(true, 1);
        next(true);
        next(true);
        claim();
        next(true);
        end();
        assert_eq!(RING.with(|r| r.current.get()), 1);
    }

    #[test]
    fn a_disabled_screen_is_inert() {
        clear();
        assert_eq!(frame(1, 3, &[]), 0);
        begin(false, 1);
        assert!(!next(true));
        end();
        assert_eq!(RING.with(|r| r.current.get()), -1);
    }
}
