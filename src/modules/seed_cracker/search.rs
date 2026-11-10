use std::collections::BTreeMap;

use seedcrack::cracker::{LIFT_BITS, LIFT_COUNT, PILLAR_BITS};
use seedcrack::{Cracker, Event, Observation, Stage};

pub use imp::Search;

pub enum Cmd {
    Add {
        observation: Observation,
        sighting: Option<(&'static str, (i32, i32))>,
    },
    Status,
    Clear,
    Restart,
}

pub enum Out {
    Added {
        label: &'static str,
        chunk: (i32, i32),
        structure_bits: f64,
        decorator_bits: f64,
    },
    Event(Event),
    Status(Vec<String>),
}

fn apply(cracker: &mut Cracker, cmd: Cmd, out: &mut Vec<Out>) {
    match cmd {
        Cmd::Add {
            observation,
            sighting,
        } => {
            if cracker.add(observation)
                && let Some((label, chunk)) = sighting
            {
                out.push(Out::Added {
                    label,
                    chunk,
                    structure_bits: cracker.structure_bits(),
                    decorator_bits: cracker.decorator_bits(),
                });
            }
        }
        Cmd::Status => out.push(Out::Status(status(cracker))),
        Cmd::Clear => cracker.clear(),
        Cmd::Restart => cracker.restart(),
    }
}

pub fn stage_name(stage: Stage) -> &'static str {
    match stage {
        Stage::Idle => "waiting for more sightings",
        Stage::LiftLow => "lifting, low 19 bits",
        Stage::LiftHigh => "lifting, high 29 bits",
        Stage::Pillars => "searching under the pillar seeds",
        Stage::WorldByHash => "matching the server's hashed seed",
        Stage::WorldByDecorators => "matching decorations",
        Stage::Done => "done",
    }
}

fn status(c: &Cracker) -> Vec<String> {
    let mut lines = vec![format!(
        "Seed Cracker ({}): {}, {:.0}%",
        c.version().name(),
        stage_name(c.stage()),
        c.progress() * 100.0
    )];

    let mut sets: BTreeMap<&str, (usize, f64)> = BTreeMap::new();
    for s in c.structures() {
        let e = sets.entry(s.set.name()).or_default();
        e.0 += 1;
        e.1 += s.set.placement().bits(s.chunk_x, s.chunk_z);
    }
    for (name, (n, bits)) in &sets {
        lines.push(format!("  {name}: {n}, {bits:.1} bits"));
    }
    lines.push(format!(
        "Structures: {:.1} bits, {} liftable (lifting wants {LIFT_COUNT} and {LIFT_BITS:.0} bits, the pillar search {PILLAR_BITS:.0} bits)",
        c.structure_bits(),
        c.liftable_count(),
    ));

    let mut decorators: BTreeMap<&str, usize> = BTreeMap::new();
    for d in c.decorators() {
        *decorators.entry(d.kind.name()).or_default() += 1;
    }
    if !decorators.is_empty() {
        let kinds: Vec<String> = decorators.iter().map(|(k, n)| format!("{k} {n}")).collect();
        lines.push(format!(
            "Decorations: {}, {:.1} bits",
            kinds.join(", "),
            c.decorator_bits()
        ));
    }
    if let Some(p) = c.pillars() {
        lines.push(format!("End pillars: {} of 10 heights", p.known()));
    }
    lines.push(match c.hashed_seed() {
        Some(h) => format!("Hashed seed: {h}"),
        None => "Hashed seed: none".to_string(),
    });

    let held = [
        ("Pillar seeds", c.pillar_seeds().map(|s| s.len())),
        ("Structure seeds", c.structure_seeds().map(|s| s.len())),
        ("World seeds", c.world_seeds().map(|s| s.len())),
    ];
    let held: Vec<String> = held
        .iter()
        .filter_map(|(name, n)| n.map(|n| format!("{name} {n}")))
        .collect();
    if !held.is_empty() {
        lines.push(format!("Held: {}", held.join(", ")));
    }
    lines
}

#[cfg(not(target_arch = "wasm32"))]
mod imp {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering::Relaxed};
    use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender, TryRecvError};
    use std::time::Duration;

    use seedcrack::{Cracker, Version};

    use super::{Cmd, Out, apply};
    use crate::modules::seed_cracker::SESSION;

    const BUDGET: u64 = 1 << 24;

    const IDLE: Duration = Duration::from_millis(500);

    pub struct Search {
        to: Sender<Cmd>,
        from: Receiver<Out>,
        cancel: Arc<AtomicBool>,
    }

    impl Search {
        pub fn start(version: Version, session: u32) -> Option<Search> {
            let (to, commands) = mpsc::channel();
            let (answers, from) = mpsc::channel();
            let cancel = Arc::new(AtomicBool::new(false));
            let stop = cancel.clone();
            let threads = std::thread::available_parallelism()
                .map_or(1, |n| n.get().saturating_sub(2).max(1));
            std::thread::Builder::new()
                .name("seed cracker".into())
                .spawn(move || {
                    work(
                        Cracker::new(version),
                        commands,
                        answers,
                        stop,
                        session,
                        threads,
                    )
                })
                .ok()?;
            Some(Search { to, from, cancel })
        }

        pub fn send(&mut self, cmd: Cmd) {
            let _ = self.to.send(cmd);
        }

        pub fn poll(&mut self) -> Vec<Out> {
            self.from.try_iter().collect()
        }
    }

    impl Drop for Search {
        fn drop(&mut self) {
            self.cancel.store(true, Relaxed);
        }
    }

    fn work(
        mut cracker: Cracker,
        commands: Receiver<Cmd>,
        answers: Sender<Out>,
        stop: Arc<AtomicBool>,
        session: u32,
        threads: usize,
    ) {
        let mut out = Vec::new();
        let mut busy = true;
        loop {
            if stop.load(Relaxed) || SESSION.load(Relaxed) != session {
                return;
            }
            if !busy {
                match commands.recv_timeout(IDLE) {
                    Ok(cmd) => apply(&mut cracker, cmd, &mut out),
                    Err(RecvTimeoutError::Timeout) => continue,
                    Err(RecvTimeoutError::Disconnected) => return,
                }
            }
            loop {
                match commands.try_recv() {
                    Ok(cmd) => apply(&mut cracker, cmd, &mut out),
                    Err(TryRecvError::Empty) => break,
                    Err(TryRecvError::Disconnected) => return,
                }
            }
            busy = cracker.step_parallel(BUDGET, threads);
            crate::modules::seed_cracker::hud::publish(&cracker);
            out.extend(cracker.drain_events().into_iter().map(Out::Event));
            for o in out.drain(..) {
                if answers.send(o).is_err() {
                    return;
                }
            }
        }
    }
}

#[cfg(target_arch = "wasm32")]
mod imp {
    use seedcrack::{Cracker, Version};

    use super::{Cmd, Out, apply};
    use crate::modules::seed_cracker::pool::Pool;

    const BUDGET: u64 = 1 << 16;

    pub struct Search {
        cracker: Cracker,
        pool: Pool,
        busy: bool,
        out: Vec<Out>,
    }

    impl Search {
        pub fn start(version: Version, _session: u32) -> Option<Search> {
            Some(Search {
                cracker: Cracker::new(version),
                pool: Pool::new(),
                busy: true,
                out: Vec::new(),
            })
        }

        pub fn send(&mut self, cmd: Cmd) {
            apply(&mut self.cracker, cmd, &mut self.out);
            self.busy = true;
        }

        pub fn poll(&mut self) -> Vec<Out> {
            if self.busy {
                crate::prof_span!("modules:seed_cracker:search");
                self.busy = match self.pool.pump(&mut self.cracker) {
                    Some(busy) => busy,
                    None => self.cracker.step(BUDGET),
                };
                crate::modules::seed_cracker::hud::publish(&self.cracker);
            }
            self.out
                .extend(self.cracker.drain_events().into_iter().map(Out::Event));
            std::mem::take(&mut self.out)
        }
    }
}
