use azalea_chat::{FormattedText, translatable_component::PrimitiveOrComponent};
use azalea_core::registry_holder::RegistryHolder;
use simdnbt::owned::{Nbt, NbtCompound, NbtTag};

use crate::play::dialog::Click;
use crate::text::events::{self, Events, Hover, SpanEvent};
use crate::text::{Span, Style};

pub fn from_nbt_tag(tag: &NbtTag) -> Option<FormattedText> {
    use simdnbt::{FromNbtTag, Mutf8String};

    let mut buf = Vec::new();
    let wrapper = NbtCompound::from_values(vec![(Mutf8String::from("m"), tag.clone())]);
    Nbt::new(Mutf8String::from(""), wrapper).write_unnamed(&mut buf);
    let nbt = simdnbt::borrow::read_unnamed(&mut std::io::Cursor::new(&buf[..])).ok()?;
    let base = match nbt {
        simdnbt::borrow::Nbt::Some(base) => base,
        simdnbt::borrow::Nbt::None => return None,
    };
    FormattedText::from_nbt_tag(base.as_compound().get("m")?)
}

pub fn to_spans(text: &FormattedText) -> Vec<Span> {
    to_spans_styled(text, Style::default())
}

pub fn to_spans_events(
    text: &FormattedText,
    registries: Option<&RegistryHolder>,
) -> (Vec<Span>, Events) {
    let mut out = Vec::new();
    let mut sink = Some(Sink {
        registries,
        table: Vec::new(),
    });
    walk(text, Style::default(), &mut out, &mut sink);
    out.retain(|s| !s.text.is_empty());
    let table = sink.map(|s| s.table).unwrap_or_default();
    let events = (!table.is_empty()).then(|| table.into());
    (out, events)
}

struct Sink<'a> {
    registries: Option<&'a RegistryHolder>,
    table: Vec<SpanEvent>,
}

pub fn to_spans_styled(text: &FormattedText, root: Style) -> Vec<Span> {
    let mut out = Vec::new();
    walk(text, root, &mut out, &mut None);
    out.retain(|s| !s.text.is_empty());
    out
}

fn push_legacy(out: &mut Vec<Span>, text: &str, style: Style) {
    if text.is_empty() {
        return;
    }
    if !text.contains('\u{a7}') {
        out.push(Span {
            text: text.to_string(),
            style,
        });
        return;
    }
    out.extend(crate::text::parse_formatted_from(text, style));
}

fn walk(text: &FormattedText, inherited: Style, out: &mut Vec<Span>, sink: &mut Option<Sink>) {
    let base = text.get_base();
    let mut style = inherited;
    let az = &base.style;
    if let Some(c) = &az.color {
        style.color = c.value & 0xFF_FFFF;
    }
    if let Some(v) = az.bold {
        style.bold = v;
    }
    if let Some(v) = az.italic {
        style.italic = v;
    }
    if let Some(v) = az.underlined {
        style.underline = v;
    }
    if let Some(v) = az.strikethrough {
        style.strikethrough = v;
    }
    if let Some(v) = az.obfuscated {
        style.obfuscated = v;
    }
    if let Some(sink) = sink.as_mut()
        && (az.click_event.is_some() || az.hover_event.is_some() || az.insertion.is_some())
    {
        let event = sink.event(az);
        style.event = events::push(&mut sink.table, event);
    }

    match text {
        FormattedText::Text(c) => {
            push_legacy(out, &c.text, style);
        }
        FormattedText::Translatable(c) => match c.read() {
            Ok(read) => {
                push_legacy(out, &read.text, style);
                for (i, sibling) in read.base.siblings.iter().enumerate() {
                    walk(
                        arg_component(c, i, sibling).unwrap_or(sibling),
                        style,
                        out,
                        sink,
                    );
                }
            }
            Err(_) => {
                let mut c = c.clone();
                c.base.siblings.clear();
                let own = c.to_string();
                push_legacy(out, &own, style);
            }
        },
    }

    for sibling in &base.siblings {
        walk(sibling, style, out, sink);
    }
}

fn arg_component<'a>(
    c: &'a azalea_chat::translatable_component::TranslatableComponent,
    i: usize,
    sibling: &FormattedText,
) -> Option<&'a FormattedText> {
    if i % 2 == 0 {
        return None;
    }
    let PrimitiveOrComponent::FormattedText(arg) = c.args.get(i / 2)? else {
        return None;
    };
    (sibling.to_string() == arg.to_string()).then_some(arg)
}

impl Sink<'_> {
    fn event(&self, az: &azalea_chat::style::Style) -> SpanEvent {
        SpanEvent {
            click: az.click_event.as_ref().and_then(|c| self.click(c)),
            hover: az.hover_event.as_ref().and_then(|h| self.hover(h)),
            insertion: az.insertion.clone(),
        }
    }

    fn click(&self, click: &azalea_chat::click_event::ClickEvent) -> Option<Click> {
        use azalea_chat::click_event::ClickEvent as Az;

        Some(match click {
            Az::OpenUrl { url } => Click::OpenUrl(url.clone()),
            Az::RunCommand { command } => Click::RunCommand(command.clone()),
            Az::SuggestCommand { command } => Click::SuggestCommand(command.clone()),
            Az::ChangePage { page } => Click::ChangePage(*page),
            Az::CopyToClipboard { value } => Click::CopyToClipboard(value.clone()),
            Az::ShowDialog { dialog } => Click::ShowDialog(
                crate::play::dialog::parse_click_dialog(dialog, self.registries?)?,
            ),
            Az::Custom { id, payload } => Click::Custom {
                id: id.clone(),
                payload: match payload {
                    Nbt::Some(base) => Some(NbtTag::Compound(base.clone().as_compound())),
                    Nbt::None => None,
                },
            },
            Az::OpenFile { .. } => return None,
        })
    }

    fn hover(&self, hover: &azalea_chat::hover_event::HoverEvent) -> Option<Hover> {
        use azalea_chat::hover_event::HoverEvent as Az;

        Some(match hover {
            Az::ShowText { value } => Hover::Lines(events::split_lines(to_spans(value))),
            Az::ShowItem { item } => {
                Hover::Item(crate::play::inventory_bridge::slot_stack_nbt(item)?)
            }
            Az::ShowEntity { id, uuid, name } => {
                let mut lines: Vec<Vec<Span>> = Vec::with_capacity(3);
                if let Some(name) = name {
                    lines.push(to_spans(name));
                }
                let (namespace, path) = id.split_once(':').unwrap_or(("minecraft", id));
                let kind =
                    crate::gui::tooltip::translate(&format!("entity.{namespace}.{path}"), &[]);
                lines.push(crate::text::styled(
                    &crate::gui::tooltip::translate("gui.entity_tooltip.type", &[kind]),
                    Style::default(),
                ));
                if let Some(uuid) = uuid {
                    lines.push(crate::text::styled(uuid, Style::default()));
                }
                Hover::Lines(lines)
            }
        })
    }
}
