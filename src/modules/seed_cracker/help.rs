use std::sync::Arc;

use seedcrack::{Dimension, FinderKind, Yields};

use crate::modules::registry::commands as prefix_setting;
use crate::modules::store;
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
                Some((set.placement().expected_bits(), super::finder_label(k)))
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
        line(crate::lang::tr!("seed_cracker.help.intro"), GRAY),
        line(crate::lang::trf!("seed_cracker.help.key", lift), YELLOW),
        line(structures(Dimension::Overworld, |liftable| liftable), GRAY),
        line(crate::lang::tr!("seed_cracker.help.extra"), AQUA),
        line(structures(Dimension::Overworld, |liftable| !liftable), GRAY),
        line(crate::lang::tr!("seed_cracker.help.end"), AQUA),
        line(crate::lang::tr!("seed_cracker.help.end_route"), GRAY),
        line(crate::lang::tr!("seed_cracker.help.decorations"), AQUA),
        line(decorations(), GRAY),
        line(crate::lang::tr!("seed_cracker.help.bedrock"), AQUA),
        line(crate::lang::tr!("seed_cracker.help.nether"), GRAY),
        line(
            crate::lang::trf!(
                "seed_cracker.help.trust",
                store().text(prefix_setting::PREFIX)
            ),
            GRAY,
        ),
        line(crate::lang::tr!("seed_cracker.help.copy"), GRAY),
    ];
    let title = vec![Span::colored(crate::lang::tr!("seed_cracker.title"), AQUA)];
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
                label: vec![Span::colored(
                    crate::lang::tr!("seed_cracker.help.done"),
                    WHITE,
                )],
                tooltip: None,
                width: 150.0,
                action: None,
            }],
        },
    };
    shared.lock().unwrap().session.dialog_show = Some(Arc::new(dialog));
}
