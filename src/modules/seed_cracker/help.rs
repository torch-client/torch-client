use std::sync::Arc;

use seedcrack::{Dimension, FinderKind, Yields};

use crate::play::dialog::{ActionButton, AfterAction, Body, Dialog, Kind};
use crate::session::SharedMutex;
use crate::text::{AQUA, GRAY, Span, WHITE, YELLOW};

const WIDTH: f32 = 320.0;

fn line(text: impl Into<String>, rgb: u32) -> Body {
    Body::Plain {
        contents: vec![Span::colored(text, rgb)],
        width: WIDTH,
    }
}

fn structures(dim: Dimension, keep: impl Fn(bool) -> bool) -> String {
    let mut list: Vec<(f64, &str)> = FinderKind::ALL
        .into_iter()
        .filter(|k| k.dimension() == dim)
        .filter_map(|k| match k.yields() {
            Yields::Structure(set) if keep(set.placement().liftable()) => {
                Some((set.placement().expected_bits(), k.name()))
            }
            _ => None,
        })
        .collect();
    list.sort_by(|a, b| b.0.total_cmp(&a.0));
    list.iter()
        .map(|(bits, name)| format!("{name} ~{bits:.1}"))
        .collect::<Vec<_>>()
        .join(", ")
}

fn decorations() -> String {
    FinderKind::ALL
        .into_iter()
        .filter(|k| matches!(k.yields(), Yields::Decorator(_)))
        .map(FinderKind::name)
        .collect::<Vec<_>>()
        .join(", ")
}

pub fn show(shared: &SharedMutex) {
    let lift = seedcrack::cracker::LIFT_COUNT;
    let body = vec![
        line(
            "Explore with the module on. Finds are read as chunks load and the seed is worked out by itself; \
             the HUD panel shows how far along it is. The numbers are bits: how much one find narrows the seed.",
            GRAY,
        ),
        line(format!("Key finds, you need {lift}:"), YELLOW),
        line(structures(Dimension::Overworld, |liftable| liftable), GRAY),
        line("Extra finds, they add bits but are not key:", AQUA),
        line(structures(Dimension::Overworld, |liftable| !liftable), GRAY),
        line("The End:", AQUA),
        line(
            "Fly past the obsidian pillars near the centre, then find end cities until the bar fills. \
             This route needs no key finds.",
            GRAY,
        ),
        line(
            "Decorations, only needed when the server hides its seed:",
            AQUA,
        ),
        line(decorations(), GRAY),
        line("The Nether has nothing to find yet.", GRAY),
        line(
            "Once found, the seed appears in chat: click it to copy.",
            GRAY,
        ),
    ];
    let title = vec![Span::colored("Seed Cracker", AQUA)];
    let dialog = Dialog {
        title: title.clone(),
        external_title: title,
        can_close_with_escape: true,
        pause: true,
        after_action: AfterAction::Close,
        body,
        inputs: Vec::new(),
        kind: Kind::Simple {
            actions: vec![ActionButton {
                label: vec![Span::colored("Done", WHITE)],
                tooltip: None,
                width: 150.0,
                action: None,
            }],
        },
    };
    shared.lock().unwrap().session.dialog_show = Some(Arc::new(dialog));
}
