use std::collections::BTreeMap;

use seedcrack::cracker::{
    BEDROCK_TARGET, LIFT_BITS, LIFT_COUNT, OVERWORLD_BEDROCK_MIN, PILLAR_BITS,
};
use seedcrack::{Band, BedrockNeed, Cracker, Event, Mode, Observation, Stage, Trust, Version};

pub use imp::Search;

pub enum Cmd {
    Add {
        observation: Observation,
        sighting: Option<(&'static str, (i32, i32))>,
    },
    Status,
    Clear,
    Restart,
    SetMode(Mode),
    SetBedrock(bool),
    Resolve(Trust),
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
    Hint {
        hint: Option<BedrockNeed>,
        cells: [usize; 3],
    },
    BandFull {
        band: Band,
        cells: [usize; 3],
    },
    Resolved(Option<Trust>),
}

fn cracker(version: Version, mode: Mode, bedrock: bool) -> Cracker {
    let mut cracker = Cracker::new(version);
    cracker.set_mode(mode);
    cracker.set_bedrock(bedrock);
    cracker
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
        Cmd::SetMode(mode) => cracker.set_mode(mode),
        Cmd::SetBedrock(on) => cracker.set_bedrock(on),
        Cmd::Resolve(trust) => {
            let open = cracker.conflict();
            cracker.resolve_conflict(trust);
            out.push(Out::Resolved(open.then_some(trust)));
        }
    }
}

pub fn bedrock_wanted(band: Band) -> usize {
    match band {
        Band::NetherFloor | Band::NetherRoof => BEDROCK_TARGET,
        Band::OverworldFloor => OVERWORLD_BEDROCK_MIN,
    }
}

#[derive(Default)]
struct Heard {
    hint: Option<BedrockNeed>,
    full: [bool; 3],
}

fn bedrock_news(c: &Cracker, heard: &mut Heard, out: &mut Vec<Out>) {
    let cells = Band::ALL.map(|b| c.bedrock_cells(b));
    let hint = c.bedrock_hint();
    if hint != heard.hint {
        heard.hint = hint;
        out.push(Out::Hint { hint, cells });
    }
    for (i, band) in Band::ALL.into_iter().enumerate() {
        let full = cells[i] >= bedrock_wanted(band);
        if full && !heard.full[i] {
            out.push(Out::BandFull { band, cells });
        }
        heard.full[i] = full;
    }
}

pub fn band_name(band: Band) -> &'static str {
    match band {
        Band::NetherFloor => crate::lang::tr!("seed_cracker.band.nether_floor"),
        Band::NetherRoof => crate::lang::tr!("seed_cracker.band.nether_roof"),
        Band::OverworldFloor => crate::lang::tr!("seed_cracker.band.overworld_floor"),
    }
}

pub fn bedrock_progress(band: Band, cells: [usize; 3]) -> String {
    match band {
        Band::NetherFloor | Band::NetherRoof => {
            crate::lang::trf!(
                "seed_cracker.hud.bedrock_nether",
                cells[0],
                BEDROCK_TARGET,
                cells[1],
                BEDROCK_TARGET
            )
        }
        Band::OverworldFloor => crate::lang::trf!(
            "seed_cracker.hud.bedrock_overworld",
            cells[2],
            OVERWORLD_BEDROCK_MIN
        ),
    }
}

pub fn hint_text(hint: Option<BedrockNeed>, cells: [usize; 3]) -> Option<String> {
    use crate::renderer::dimension::{self, Dimension};
    let here = dimension::current();
    Some(match hint? {
        BedrockNeed::Collecting => bedrock_progress(Band::NetherFloor, cells),
        BedrockNeed::VisitNether if here == Dimension::Nether => {
            bedrock_progress(Band::NetherFloor, cells)
        }
        BedrockNeed::VisitNether => crate::lang::tr!("seed_cracker.hud.visit_nether").to_string(),
        BedrockNeed::VisitOverworld if here == Dimension::Overworld => {
            bedrock_progress(Band::OverworldFloor, cells)
        }
        BedrockNeed::VisitOverworld => {
            crate::lang::tr!("seed_cracker.hud.visit_overworld").to_string()
        }
    })
}

fn mode_name(mode: Mode) -> &'static str {
    use crate::modules::registry::Options;
    use crate::modules::seed_cracker::CrackMode;
    let shown = match mode {
        Mode::Normal => CrackMode::Normal,
        Mode::Fast => CrackMode::Fast,
    };
    crate::modules::registry::option_label(CrackMode::NAMES, CrackMode::KEYS, shown as usize)
}

pub fn stage_name(stage: Stage) -> &'static str {
    match stage {
        Stage::Idle => crate::lang::tr!("seed_cracker.stage.idle"),
        Stage::Bedrock => crate::lang::tr!("seed_cracker.stage.bedrock"),
        Stage::LiftLow => crate::lang::tr!("seed_cracker.stage.lift_low"),
        Stage::LiftHigh => crate::lang::tr!("seed_cracker.stage.lift_high"),
        Stage::Pillars => crate::lang::tr!("seed_cracker.stage.pillars"),
        Stage::WorldByHash => crate::lang::tr!("seed_cracker.stage.world_by_hash"),
        Stage::WorldByDecorators => crate::lang::tr!("seed_cracker.stage.world_by_decorators"),
        Stage::Done => crate::lang::tr!("seed_cracker.stage.done"),
    }
}

fn status(c: &Cracker) -> Vec<String> {
    let mut lines = vec![crate::lang::trf!(
        "seed_cracker.status.head",
        c.version().name(),
        stage_name(c.stage()),
        format_args!("{:.0}", c.progress() * 100.0)
    )];

    let mut sets: BTreeMap<&str, (usize, f64)> = BTreeMap::new();
    for s in c.structures() {
        let e = sets.entry(s.set.name()).or_default();
        e.0 += 1;
        e.1 += s.set.placement().bits(s.chunk_x, s.chunk_z);
    }
    for (name, (n, bits)) in &sets {
        let line = crate::lang::trf!(
            "seed_cracker.status.set",
            name,
            n,
            format_args!("{bits:.1}")
        );
        lines.push(format!("  {line}"));
    }
    lines.push(crate::lang::trf!(
        "seed_cracker.status.structures",
        format_args!("{:.1}", c.structure_bits()),
        c.liftable_count(),
        LIFT_COUNT,
        format_args!("{LIFT_BITS:.0}"),
        format_args!("{PILLAR_BITS:.0}"),
    ));

    let mut decorators: BTreeMap<&str, usize> = BTreeMap::new();
    for d in c.decorators() {
        *decorators
            .entry(super::decorator_label(d.kind))
            .or_default() += 1;
    }
    if !decorators.is_empty() {
        let kinds: Vec<String> = decorators.iter().map(|(k, n)| format!("{k} {n}")).collect();
        let line = crate::lang::trf!(
            "seed_cracker.status.decorations",
            kinds.join(", "),
            format_args!("{:.1}", c.decorator_bits())
        );
        lines.push(line);
    }
    if let Some(p) = c.pillars() {
        lines.push(crate::lang::trf!("seed_cracker.status.pillars", p.known()));
    }
    lines.push(match c.hashed_seed() {
        Some(h) => crate::lang::trf!("seed_cracker.status.hash", h),
        None => crate::lang::tr!("seed_cracker.status.hash_none").to_string(),
    });
    lines.push(crate::lang::trf!(
        "seed_cracker.status.mode",
        mode_name(c.mode())
    ));
    if c.bedrock_enabled() {
        let cells: Vec<String> = Band::ALL
            .into_iter()
            .map(|b| {
                format!(
                    "{} {}/{}",
                    band_name(b),
                    c.bedrock_cells(b),
                    bedrock_wanted(b)
                )
            })
            .collect();
        lines.push(crate::lang::trf!(
            "seed_cracker.status.bedrock",
            cells.join(", ")
        ));
        if let Some(text) = hint_text(c.bedrock_hint(), Band::ALL.map(|b| c.bedrock_cells(b))) {
            lines.push(format!("  {text}"));
        }
        if c.conflict() {
            lines.push(format!(
                "  {}",
                crate::lang::tr!("seed_cracker.hud.conflict")
            ));
        }
    } else {
        lines.push(crate::lang::tr!("seed_cracker.status.bedrock_off").to_string());
    }

    let held: Vec<String> = [
        c.pillar_seeds()
            .map(|s| crate::lang::trf!("seed_cracker.status.held.pillar", s.len())),
        c.bedrock_seeds(Band::NetherFloor)
            .map(|n| crate::lang::trf!("seed_cracker.status.held.floor", n)),
        c.bedrock_seeds(Band::NetherRoof)
            .map(|n| crate::lang::trf!("seed_cracker.status.held.roof", n)),
        c.bedrock_structure_seeds()
            .map(|s| crate::lang::trf!("seed_cracker.status.held.bedrock", s.len())),
        c.structure_seeds()
            .map(|s| crate::lang::trf!("seed_cracker.status.held.structure", s.len())),
        c.world_seeds()
            .map(|s| crate::lang::trf!("seed_cracker.status.held.world", s.len())),
    ]
    .into_iter()
    .flatten()
    .collect();
    if !held.is_empty() {
        lines.push(crate::lang::trf!(
            "seed_cracker.status.held",
            held.join(", ")
        ));
    }
    lines
}

#[cfg(not(target_arch = "wasm32"))]
mod imp {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering::Relaxed};
    use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender, TryRecvError};
    use std::time::Duration;

    use seedcrack::{Cracker, Mode, Version};

    use super::{Cmd, Heard, Out, apply, bedrock_news};

    const BUDGET: u64 = 1 << 24;

    const IDLE: Duration = Duration::from_millis(500);

    pub struct Search {
        to: Sender<Cmd>,
        from: Receiver<Out>,
        cancel: Arc<AtomicBool>,
        paused: Arc<AtomicBool>,
    }

    impl Search {
        pub fn start(version: Version, mode: Mode, bedrock: bool) -> Option<Search> {
            let (to, commands) = mpsc::channel();
            let (answers, from) = mpsc::channel();
            let cancel = Arc::new(AtomicBool::new(false));
            let stop = cancel.clone();
            let paused = Arc::new(AtomicBool::new(false));
            let hold = paused.clone();
            let threads = std::thread::available_parallelism()
                .map_or(1, |n| n.get().saturating_sub(2).max(1));
            std::thread::Builder::new()
                .name("seed cracker".into())
                .spawn(move || {
                    work(
                        super::cracker(version, mode, bedrock),
                        commands,
                        answers,
                        stop,
                        hold,
                        threads,
                    )
                })
                .ok()?;
            Some(Search {
                to,
                from,
                cancel,
                paused,
            })
        }

        pub fn pause(&mut self, on: bool) {
            self.paused.store(on, Relaxed);
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
        paused: Arc<AtomicBool>,
        threads: usize,
    ) {
        let mut out = Vec::new();
        let mut heard = Heard::default();
        let mut busy = true;
        loop {
            if stop.load(Relaxed) {
                return;
            }
            if paused.load(Relaxed) {
                match commands.recv_timeout(IDLE) {
                    Ok(cmd) => {
                        apply(&mut cracker, cmd, &mut out);
                        busy = true;
                    }
                    Err(RecvTimeoutError::Timeout) => {}
                    Err(RecvTimeoutError::Disconnected) => return,
                }
                continue;
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
            bedrock_news(&cracker, &mut heard, &mut out);
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
    use seedcrack::{Cracker, Mode, Version};

    use super::{Cmd, Heard, Out, apply, bedrock_news};
    use crate::modules::seed_cracker::pool::Pool;

    const BUDGET: u64 = 1 << 16;

    pub struct Search {
        cracker: Cracker,
        pool: Pool,
        busy: bool,
        out: Vec<Out>,
        heard: Heard,
    }

    impl Search {
        pub fn start(version: Version, mode: Mode, bedrock: bool) -> Option<Search> {
            Some(Search {
                cracker: super::cracker(version, mode, bedrock),
                pool: Pool::new(),
                busy: true,
                out: Vec::new(),
                heard: Heard::default(),
            })
        }

        pub fn send(&mut self, cmd: Cmd) {
            apply(&mut self.cracker, cmd, &mut self.out);
            self.busy = true;
        }

        pub fn pause(&mut self, _on: bool) {}

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
            bedrock_news(&self.cracker, &mut self.heard, &mut self.out);
            std::mem::take(&mut self.out)
        }
    }
}
