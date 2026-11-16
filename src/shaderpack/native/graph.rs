use std::collections::{HashMap, HashSet};

use super::api::Group;
use super::manifest::{self, Anchor, Layout, Named, Target};
use super::options::Values;
use super::problem::{Position, Problem};

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) enum Module {
    Pass(String),
    Geometry(Group),
    Shadow,
}

pub(crate) type Reads = HashMap<Module, HashSet<String>>;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct Plan {
    pub(crate) targets: Vec<PlanTarget>,
    pub(crate) shadow: Option<PlanShadow>,
    pub(crate) opaque: Option<PlanGroup>,
    pub(crate) translucent: Option<PlanGroup>,
    pub(crate) passes: Vec<PlanPass>,
}

impl Plan {
    #[allow(
        dead_code,
        reason = "phase 2 of docs/SHADER_FORMAT.md records each anchor's passes"
    )]
    pub(crate) fn at(&self, anchor: Anchor) -> impl Iterator<Item = &PlanPass> {
        self.passes.iter().filter(move |p| p.anchor == anchor)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct PlanTarget {
    pub(crate) target: usize,
    pub(crate) history: bool,
    pub(crate) flip: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct Access {
    pub(crate) writes: Vec<usize>,
    pub(crate) reads: Vec<usize>,
    pub(crate) reads_prev: Vec<usize>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct PlanPass {
    pub(crate) pass: usize,
    pub(crate) anchor: Anchor,
    pub(crate) access: Access,
    pub(crate) flips: Vec<usize>,
    pub(crate) writes_screen: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct PlanGroup {
    pub(crate) kind: Group,
    pub(crate) access: Access,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct PlanShadow {
    pub(crate) color: bool,
    pub(crate) access: Access,
}

#[derive(Clone, Copy)]
enum Step {
    Pass(usize),
    Shadow,
    Geometry(Group),
}

pub(crate) fn plan(layout: Layout<'_>, values: &Values, reads: &Reads) -> (Plan, Vec<Problem>) {
    let mut walk = Walk {
        dimension: layout.dimension,
        problems: Vec::new(),
        index: HashMap::new(),
        absent: HashSet::new(),
    };
    let mut plan = Plan::default();

    for (i, target) in layout.targets.iter().enumerate() {
        if target.when.as_ref().is_none_or(|c| c.eval(values)) {
            walk.index
                .insert(target.id.as_str(), (plan.targets.len(), target));
            plan.targets.push(PlanTarget {
                target: i,
                history: target.history,
                flip: false,
            });
        } else {
            walk.absent.insert(target.id.as_str());
        }
    }

    let passes_at = |anchor: Anchor| -> Vec<Step> {
        let runs = |pass: &manifest::Pass| pass.when.as_ref().is_none_or(|c| c.eval(values));
        let at = layout
            .passes
            .iter()
            .enumerate()
            .filter(|(_, pass)| pass.anchor == anchor && runs(*pass));
        at.map(|(i, _)| Step::Pass(i)).collect()
    };
    let shadow_runs = layout
        .shadow
        .is_some_and(|s| s.when.as_ref().is_none_or(|c| c.eval(values)));
    let mut steps: Vec<Step> = passes_at(Anchor::Begin);
    steps.extend(shadow_runs.then_some(Step::Shadow));
    steps.extend(
        layout
            .geometry
            .opaque
            .is_some()
            .then_some(Step::Geometry(Group::Opaque)),
    );
    steps.extend(passes_at(Anchor::Deferred));
    steps.extend(
        layout
            .geometry
            .translucent
            .is_some()
            .then_some(Step::Geometry(Group::Translucent)),
    );
    steps.extend(passes_at(Anchor::Composite));
    steps.extend(passes_at(Anchor::Final));
    let screen_writer = layout
        .passes
        .iter()
        .rposition(|p| p.anchor == Anchor::Final);

    let no_writes: &[Named] = &[];
    let mut written = vec![false; plan.targets.len()];
    let mut read = vec![false; plan.targets.len()];
    for step in steps {
        let (module, writes, at, what): (Module, &[Named], Position, String) = match step {
            Step::Pass(i) => {
                let pass = &layout.passes[i];
                (
                    Module::Pass(pass.id.clone()),
                    pass.writes.as_slice(),
                    pass.at,
                    format!("pass `{}`", pass.id),
                )
            }
            Step::Shadow => {
                let shadow = layout.shadow.expect("a shadow step has a [shadow]");
                (Module::Shadow, no_writes, shadow.at, "[shadow]".to_owned())
            }
            Step::Geometry(kind) => {
                let group = match kind {
                    Group::Opaque => layout.geometry.opaque.as_ref(),
                    Group::Translucent => layout.geometry.translucent.as_ref(),
                }
                .expect("a geometry step has its group");
                (
                    Module::Geometry(kind),
                    group.writes.as_slice(),
                    group.at,
                    format!("[geometry.{}]", kind.name()),
                )
            }
        };
        let access = walk.access(reads.get(&module), writes, at, &what);

        let mut flips = Vec::new();
        for &t in &access.reads {
            read[t] = true;
            let target = &layout.targets[plan.targets[t].target];
            if access.writes.contains(&t) {
                if matches!(step, Step::Geometry(_)) {
                    walk.report(at, format!("{what} reads `{}`, which it writes", target.id));
                    continue;
                }
                plan.targets[t].flip = true;
                flips.push(t);
            }
            if !written[t] && target.clear.is_none() && !target.history {
                walk.report_hint(
                    at,
                    format!(
                        "{what} reads `{}` before anything writes it this frame",
                        target.id
                    ),
                    "write it in an earlier step, or give it `clear` or `history`",
                );
            }
        }
        for &t in &access.reads_prev {
            read[t] = true;
        }
        for &t in &access.writes {
            written[t] = true;
        }

        match step {
            Step::Pass(i) => plan.passes.push(PlanPass {
                pass: i,
                anchor: layout.passes[i].anchor,
                access,
                flips,
                writes_screen: Some(i) == screen_writer,
            }),
            Step::Shadow => {
                let color = layout.shadow.is_some_and(|s| s.color);
                plan.shadow = Some(PlanShadow { color, access });
            }
            Step::Geometry(Group::Opaque) => {
                plan.opaque = Some(PlanGroup {
                    kind: Group::Opaque,
                    access,
                })
            }
            Step::Geometry(Group::Translucent) => {
                plan.translucent = Some(PlanGroup {
                    kind: Group::Translucent,
                    access,
                });
            }
        }
    }

    for (t, planned) in plan.targets.iter().enumerate() {
        if !read[t] {
            let target = &layout.targets[planned.target];
            walk.report_hint(
                target.at,
                format!("nothing reads target `{}`", target.id),
                "remove it, or read it in a step",
            );
        }
    }
    (plan, walk.problems)
}

struct Walk<'a> {
    dimension: Option<&'a str>,
    problems: Vec<Problem>,
    index: HashMap<&'a str, (usize, &'a Target)>,
    absent: HashSet<&'a str>,
}

impl Walk<'_> {
    fn report(&mut self, at: Position, message: String) {
        let message = match self.dimension {
            Some(dimension) => format!("{message}, in {dimension}"),
            None => message,
        };
        self.problems.push(Problem {
            file: manifest::FILE.to_owned(),
            at: Some(at),
            message,
            hint: None,
        });
    }

    fn report_hint(&mut self, at: Position, message: String, hint: &str) {
        self.report(at, message);
        if let Some(problem) = self.problems.last_mut() {
            problem.hint = Some(hint.to_owned());
        }
    }

    fn access(
        &mut self,
        names: Option<&HashSet<String>>,
        writes: &[Named],
        at: Position,
        what: &str,
    ) -> Access {
        let mut access = Access::default();
        let mut names: Vec<&str> = names.into_iter().flatten().map(String::as_str).collect();
        names.sort_unstable();
        for name in names {
            let prev = name.strip_suffix("_prev");
            if let Some(&(t, _)) = self.index.get(name) {
                access.reads.push(t);
            } else if let Some(&(t, target)) = prev.and_then(|base| self.index.get(base))
                && target.history
            {
                access.reads_prev.push(t);
            } else if self.absent.contains(name)
                || prev.is_some_and(|base| self.absent.contains(base))
            {
                self.report(
                    at,
                    format!("{what} reads `{name}`, which does not exist with the current options"),
                );
            }
        }
        for named in writes {
            if let Some(&(t, _)) = self.index.get(named.id.as_str()) {
                access.writes.push(t);
            } else if self.absent.contains(named.id.as_str()) {
                self.report(
                    named.at,
                    format!(
                        "{what} writes `{}`, which does not exist with the current options",
                        named.id
                    ),
                );
            }
        }
        access
    }
}

#[cfg(test)]
mod tests {
    use super::super::manifest::Manifest;
    use super::super::manifest::tests::{EXAMPLE, parse_ok};
    use super::super::options::Value;
    use super::*;

    fn reads(list: &[(Module, &[&str])]) -> Reads {
        list.iter()
            .map(|(module, names)| {
                (
                    module.clone(),
                    names.iter().map(|n| (*n).to_owned()).collect(),
                )
            })
            .collect()
    }

    fn pass(id: &str) -> Module {
        Module::Pass(id.to_owned())
    }

    fn example_reads() -> Reads {
        reads(&[
            (
                pass("lighting"),
                &["albedo", "normal", "frame", "linear_clamp"],
            ),
            (pass("tonemap"), &["lit", "nearest_clamp"]),
        ])
    }

    fn messages(problems: &[Problem]) -> Vec<&str> {
        problems.iter().map(|p| p.message.as_str()).collect()
    }

    #[test]
    fn the_example_plans_cleanly() {
        let m = parse_ok(EXAMPLE);
        let values = Values::defaults(&m.options);
        let (plan, problems) = plan(m.layout("overworld"), &values, &example_reads());
        assert!(problems.is_empty(), "{problems:#?}");
        assert_eq!(plan.targets.len(), 3);
        assert!(plan.targets.iter().all(|t| !t.flip));
        assert!(plan.targets[2].history);
        assert!(plan.shadow.is_some());
        assert_eq!(plan.opaque.as_ref().unwrap().access.writes, [0, 1]);
        let ids: Vec<usize> = plan.passes.iter().map(|p| p.pass).collect();
        assert_eq!(ids, [0, 1]);
        assert_eq!(plan.at(Anchor::Final).count(), 1);
        assert!(plan.passes[1].writes_screen && !plan.passes[0].writes_screen);
        assert_eq!(plan.passes[0].access.reads, [0, 1]);
    }

    #[test]
    fn a_condition_turns_the_shadow_off() {
        let m = parse_ok(EXAMPLE);
        let mut values = Values::defaults(&m.options);
        values
            .set(&m.options, "shadows", Value::Bool(false))
            .unwrap();
        let (plan, problems) = plan(m.layout("overworld"), &values, &example_reads());
        assert!(problems.is_empty(), "{problems:#?}");
        assert!(plan.shadow.is_none());
    }

    #[test]
    fn a_pass_reading_its_own_output_flips() {
        let m = parse_ok(EXAMPLE);
        let values = Values::defaults(&m.options);
        let reads = reads(&[
            (pass("lighting"), &["albedo", "normal", "lit"]),
            (pass("tonemap"), &["lit"]),
        ]);
        let (plan, problems) = plan(m.layout("overworld"), &values, &reads);
        assert!(problems.is_empty(), "{problems:#?}");
        assert_eq!(plan.passes[0].flips, [2]);
        assert!(plan.targets[2].flip);
    }

    #[test]
    fn reading_before_any_write_is_a_problem() {
        let text = format!(
            "{EXAMPLE}\n[[pass]]\nid = \"pre\"\nat = \"begin\"\nfile = \"pre.wgsl\"\nwrites = [\"lit\"]\n"
        );
        let m = parse_ok(&text);
        let values = Values::defaults(&m.options);
        let mut reads = example_reads();
        reads.insert(pass("pre"), ["albedo".to_owned()].into());
        let (_, problems) = plan(m.layout("overworld"), &values, &reads);
        assert_eq!(
            messages(&problems),
            ["pass `pre` reads `albedo` before anything writes it this frame"]
        );
        assert_eq!(problems[0].at.map(|p| p.line), Some(67));
    }

    #[test]
    fn a_target_nothing_reads_is_a_problem() {
        let m = parse_ok(EXAMPLE);
        let values = Values::defaults(&m.options);
        let reads = reads(&[(pass("lighting"), &["albedo"]), (pass("tonemap"), &["lit"])]);
        let (_, problems) = plan(m.layout("overworld"), &values, &reads);
        assert_eq!(messages(&problems), ["nothing reads target `normal`"]);
    }

    #[test]
    fn history_reads_count() {
        let m = parse_ok(EXAMPLE);
        let values = Values::defaults(&m.options);
        let reads = reads(&[
            (pass("lighting"), &["albedo", "normal", "lit_prev"]),
            (pass("tonemap"), &["lit"]),
        ]);
        let (plan, problems) = plan(m.layout("overworld"), &values, &reads);
        assert!(problems.is_empty(), "{problems:#?}");
        assert_eq!(plan.passes[0].access.reads_prev, [2]);
    }

    #[test]
    fn a_geometry_group_cannot_read_what_it_writes() {
        let m = parse_ok(EXAMPLE);
        let values = Values::defaults(&m.options);
        let mut reads = example_reads();
        reads.insert(
            Module::Geometry(Group::Opaque),
            ["normal".to_owned()].into(),
        );
        let (_, problems) = plan(m.layout("overworld"), &values, &reads);
        assert_eq!(
            messages(&problems),
            ["[geometry.opaque] reads `normal`, which it writes"]
        );
    }

    #[test]
    fn a_target_switched_off_cannot_be_used() {
        let extra = "\n[[target]]\nid = \"bloom\"\nwhen = \"!shadows\"\n\n[[pass]]\nid = \"blur\"\nat = \"composite\"\nfile = \"blur.wgsl\"\nwrites = [\"bloom\"]\n";
        let m: Manifest = parse_ok(&format!("{EXAMPLE}{extra}"));
        let mut values = Values::defaults(&m.options);
        let mut reads = example_reads();
        reads.insert(
            pass("tonemap"),
            ["lit".to_owned(), "bloom".to_owned()].into(),
        );
        let (_, problems) = plan(m.layout("overworld"), &values, &reads);
        assert_eq!(
            messages(&problems),
            [
                "pass `blur` writes `bloom`, which does not exist with the current options",
                "pass `tonemap` reads `bloom`, which does not exist with the current options",
            ]
        );
        values
            .set(&m.options, "shadows", Value::Bool(false))
            .unwrap();
        let (plan, problems) = plan(m.layout("overworld"), &values, &reads);
        assert!(problems.is_empty(), "{problems:#?}");
        assert_eq!(plan.targets.len(), 4);
    }
}
