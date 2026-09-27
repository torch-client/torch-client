use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

use azalea_core::registry_holder::RegistryHolder;
use azalea_registry::identifier::Identifier;
use simdnbt::owned::{NbtCompound, NbtList, NbtTag};

use crate::session::SlotStack;
use crate::text::Span;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum AfterAction {
    #[default]
    Close,
    None,
    WaitForResponse,
}

impl AfterAction {
    fn parse(name: &str) -> AfterAction {
        match name {
            "none" => AfterAction::None,
            "wait_for_response" => AfterAction::WaitForResponse,
            _ => AfterAction::Close,
        }
    }
}

#[derive(Debug)]
pub struct Dialog {
    pub title: Vec<Span>,
    pub external_title: Vec<Span>,
    pub can_close_with_escape: bool,
    pub pause: bool,
    pub after_action: AfterAction,
    pub body: Vec<Body>,
    pub inputs: Vec<Input>,
    pub kind: Kind,
}

#[derive(Debug)]
pub enum Kind {
    Simple {
        actions: Vec<ActionButton>,
    },
    ButtonList {
        buttons: Vec<ActionButton>,
        exit: Option<ActionButton>,
        columns: u32,
    },
    ServerLinks {
        exit: Option<ActionButton>,
        columns: u32,
        button_width: f32,
    },
}

#[derive(Debug)]
pub enum Body {
    Plain {
        contents: Vec<Span>,
        width: f32,
    },
    Item {
        stack: SlotStack,
        description: Option<(Vec<Span>, f32)>,
        show_decorations: bool,
        show_tooltip: bool,
        width: f32,
        height: f32,
    },
}

#[derive(Debug)]
pub struct Input {
    pub key: String,
    pub control: Control,
}

#[derive(Debug)]
pub enum Control {
    Text {
        width: f32,
        label: Vec<Span>,
        label_visible: bool,
        initial: String,
        max_length: usize,
        multiline: Option<Multiline>,
    },
    SingleOption {
        width: f32,
        label: Vec<Span>,
        label_visible: bool,
        options: Vec<Choice>,
        initial: usize,
    },
    Boolean {
        label: Vec<Span>,
        initial: bool,
        on_true: String,
        on_false: String,
    },
    NumberRange {
        width: f32,
        label: Vec<Span>,
        label_format: String,
        range: RangeInfo,
    },
}

#[derive(Debug)]
pub struct Multiline {
    pub max_lines: Option<usize>,
    pub height: f32,
}

#[derive(Debug)]
pub struct Choice {
    pub id: String,
    pub display: Vec<Span>,
}

#[derive(Clone, Copy, Debug)]
pub struct RangeInfo {
    pub start: f32,
    pub end: f32,
    pub initial: Option<f32>,
    pub step: Option<f32>,
}

impl RangeInfo {
    fn initial_scaled(&self) -> f32 {
        self.initial.unwrap_or((self.start + self.end) / 2.0)
    }

    fn to_slider(&self, value: f32) -> f32 {
        if self.start == self.end {
            0.5
        } else {
            (value - self.start) / (self.end - self.start)
        }
    }

    pub fn initial_slider(&self) -> f32 {
        self.to_slider(self.initial_scaled())
    }

    pub fn scaled(&self, slider: f32) -> f32 {
        let in_range = self.start + (self.end - self.start) * slider;
        let Some(step) = self.step else {
            return in_range;
        };
        let initial = self.initial_scaled();
        let steps = ((in_range - initial) / step).round();
        let result = initial + steps * step;
        let position = self.to_slider(result);
        if (0.0..=1.0).contains(&position) {
            return result;
        }
        let back = steps - steps.signum();
        initial + back * step
    }

    pub fn value_string(value: f32) -> String {
        if value as i32 as f32 == value {
            (value as i32).to_string()
        } else {
            value.to_string()
        }
    }
}

#[derive(Debug)]
pub struct ActionButton {
    pub label: Vec<Span>,
    pub tooltip: Option<Vec<Span>>,
    pub width: f32,
    pub action: Option<Action>,
}

#[derive(Debug)]
pub enum Action {
    Static(Click),
    CommandTemplate(Template),
    Custom { id: String, additions: NbtCompound },
}

#[derive(Clone, Debug)]
pub enum Click {
    OpenUrl(String),
    RunCommand(String),
    SuggestCommand(String),
    ChangePage(i32),
    CopyToClipboard(String),
    ShowDialog(Arc<Dialog>),
    Custom { id: String, payload: Option<NbtTag> },
}

static SUBMIT_PENDING: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

pub fn queue_submit(shared: &crate::session::SharedMutex, submit: Submit) {
    shared.lock().unwrap().session.dialog_submits.push(submit);
    SUBMIT_PENDING.store(true, std::sync::atomic::Ordering::Release);
}

pub fn take_submits(shared: &crate::session::SharedMutex) -> Vec<Submit> {
    if !SUBMIT_PENDING.swap(false, std::sync::atomic::Ordering::Acquire) {
        return Vec::new();
    }
    std::mem::take(&mut shared.lock().unwrap().session.dialog_submits)
}

#[derive(Clone, Debug)]
pub enum Submit {
    RunCommand(String),
    Custom { id: String, payload: Option<NbtTag> },
}

#[derive(Clone, Debug)]
pub enum Value {
    Text(String),
    Boolean {
        selected: bool,
        on_true: String,
        on_false: String,
    },
    Choice(String),
    Number(f32),
}

impl Value {
    pub fn substitution(&self) -> String {
        match self {
            Value::Text(s) => escape_without_quotes(s),
            Value::Boolean {
                selected,
                on_true,
                on_false,
            } => {
                if *selected {
                    on_true.clone()
                } else {
                    on_false.clone()
                }
            }
            Value::Choice(id) => id.clone(),
            Value::Number(v) => RangeInfo::value_string(*v),
        }
    }

    pub fn tag(&self) -> NbtTag {
        match self {
            Value::Text(s) => NbtTag::String(s.as_str().into()),
            Value::Boolean { selected, .. } => NbtTag::Byte(*selected as i8),
            Value::Choice(id) => NbtTag::String(id.as_str().into()),
            Value::Number(v) => NbtTag::Float(*v),
        }
    }
}

pub fn escape_without_quotes(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for c in input.chars() {
        match c {
            '"' | '\'' | '\\' => {
                out.push('\\');
                out.push(c);
            }
            '\u{8}' => out.push_str("\\b"),
            '\t' => out.push_str("\\t"),
            '\n' => out.push_str("\\n"),
            '\u{c}' => out.push_str("\\f"),
            '\r' => out.push_str("\\r"),
            c if c < ' ' => out.push_str(&format!("\\x{:02x}", c as u32)),
            c => out.push(c),
        }
    }
    out
}

impl Action {
    pub fn resolve(&self, values: &HashMap<String, Value>) -> Option<Click> {
        match self {
            Action::Static(click) => Some(click.clone()),
            Action::CommandTemplate(template) => {
                let args: HashMap<String, String> = values
                    .iter()
                    .map(|(k, v)| (k.clone(), v.substitution()))
                    .collect();
                Some(Click::RunCommand(template.instantiate(&args)))
            }
            Action::Custom { id, additions } => {
                let mut payload = additions.clone();
                for (key, value) in values {
                    payload.remove(key.as_str());
                    payload.insert(key.as_str(), value.tag());
                }
                Some(Click::Custom {
                    id: id.clone(),
                    payload: Some(NbtTag::Compound(payload)),
                })
            }
        }
    }
}

#[derive(Debug, Default)]
pub struct Template {
    segments: Vec<String>,
    variables: Vec<String>,
}

impl Template {
    pub fn parse(input: &str) -> Option<Template> {
        let chars: Vec<char> = input.chars().collect();
        let mut segments = Vec::new();
        let mut variables = Vec::new();
        let mut start = 0;
        let mut i = 0;
        while i < chars.len() {
            if chars[i] != '$' || i + 1 >= chars.len() || chars[i + 1] != '(' {
                i += 1;
                continue;
            }
            let end = (i + 2..chars.len()).find(|&j| chars[j] == ')')?;
            let name: String = chars[i + 2..end].iter().collect();
            if !is_valid_variable_name(&name) {
                return None;
            }
            segments.push(chars[start..i].iter().collect());
            variables.push(name);
            start = end + 1;
            i = start;
        }
        if start == 0 {
            return None;
        }
        if start != chars.len() {
            segments.push(chars[start..].iter().collect());
        }
        Some(Template {
            segments,
            variables,
        })
    }

    pub fn instantiate(&self, args: &HashMap<String, String>) -> String {
        let mut out = String::new();
        for (i, variable) in self.variables.iter().enumerate() {
            out.push_str(&self.segments[i]);
            out.push_str(args.get(variable).map(String::as_str).unwrap_or(""));
        }
        if self.segments.len() > self.variables.len() {
            out.push_str(self.segments.last().unwrap());
        }
        out
    }
}

fn is_valid_variable_name(name: &str) -> bool {
    name.chars().all(|c| c.is_alphanumeric() || c == '_')
}

fn tags() -> &'static Mutex<HashMap<String, Vec<i32>>> {
    static TAGS: OnceLock<Mutex<HashMap<String, Vec<i32>>>> = OnceLock::new();
    TAGS.get_or_init(Default::default)
}

pub fn set_tags(entries: impl Iterator<Item = (String, Vec<i32>)>) {
    *tags().lock().unwrap() = entries.collect();
}

pub fn clear_tags() {
    tags().lock().unwrap().clear();
}

const MAX_DEPTH: usize = 16;

struct Ctx<'a> {
    registries: &'a RegistryHolder,
    depth: usize,
}

impl Ctx<'_> {
    fn registry_entry(&self, protocol_id: i32) -> Option<&NbtCompound> {
        let registry = self.registries.extra.get(&dialog_registry_id())?;
        registry
            .map
            .get_index(protocol_id as usize)
            .map(|(_, entry)| entry)
    }

    fn registry_entry_by_name(&self, id: &str) -> Option<&NbtCompound> {
        let registry = self.registries.extra.get(&dialog_registry_id())?;
        registry.map.get(&Identifier::new(id))
    }
}

fn dialog_registry_id() -> Identifier {
    Identifier::new("dialog")
}

pub fn parse_holder(
    holder: &azalea_registry::Holder<azalea_registry::data::Dialog, simdnbt::owned::Nbt>,
    registries: &RegistryHolder,
) -> Option<Arc<Dialog>> {
    use azalea_registry::{DataRegistry, Holder};

    let mut ctx = Ctx {
        registries,
        depth: 0,
    };
    match holder {
        Holder::Reference(dialog) => {
            let entry = ctx.registry_entry(dialog.protocol_id() as i32)?.clone();
            parse(&entry, &mut ctx)
        }
        Holder::Direct(nbt) => parse_nbt(nbt, &mut ctx),
    }
}

pub fn parse_click_dialog(tag: &NbtTag, registries: &RegistryHolder) -> Option<Arc<Dialog>> {
    sub_dialog(
        tag,
        &mut Ctx {
            registries,
            depth: 0,
        },
    )
}

pub fn parse_direct(nbt: &simdnbt::owned::Nbt, registries: &RegistryHolder) -> Option<Arc<Dialog>> {
    parse_nbt(
        nbt,
        &mut Ctx {
            registries,
            depth: 0,
        },
    )
}

fn parse_nbt(nbt: &simdnbt::owned::Nbt, ctx: &mut Ctx) -> Option<Arc<Dialog>> {
    match nbt {
        simdnbt::owned::Nbt::Some(base) => parse(&base.clone().as_compound(), ctx),
        simdnbt::owned::Nbt::None => None,
    }
}

fn parse(tag: &NbtCompound, ctx: &mut Ctx) -> Option<Arc<Dialog>> {
    if ctx.depth > MAX_DEPTH {
        crate::log_warn!("dialog", "dialog nesting deeper than {MAX_DEPTH}; stopping");
        return None;
    }
    let kind_id = tag.string("type").map(|s| s.to_string())?;
    let title = component(tag.get("title")).unwrap_or_default();
    let external_title = component(tag.get("external_title")).unwrap_or_else(|| title.clone());
    let kind = match strip_namespace(&kind_id) {
        "notice" => Kind::Simple {
            actions: vec![
                tag.compound("action")
                    .and_then(|a| action_button(a, ctx))
                    .unwrap_or_else(|| ActionButton {
                        label: crate::text::styled("Ok", crate::text::Style::default()),
                        tooltip: None,
                        width: DEFAULT_BUTTON_WIDTH,
                        action: None,
                    }),
            ],
        },
        "confirmation" => Kind::Simple {
            actions: vec![
                action_button(tag.compound("yes")?, ctx)?,
                action_button(tag.compound("no")?, ctx)?,
            ],
        },
        "multi_action" => {
            let buttons: Vec<ActionButton> = compound_list(tag.get("actions"))
                .iter()
                .filter_map(|entry| action_button(entry, ctx))
                .collect();
            if buttons.is_empty() {
                return None;
            }
            Kind::ButtonList {
                buttons,
                exit: exit_action(tag, ctx),
                columns: columns(tag),
            }
        }
        "dialog_list" => {
            let width = width(tag, "button_width", DEFAULT_BUTTON_WIDTH);
            let buttons = tag
                .get("dialogs")
                .map(|dialogs| dialog_list_buttons(dialogs, width, ctx))
                .unwrap_or_default();
            Kind::ButtonList {
                buttons,
                exit: exit_action(tag, ctx),
                columns: columns(tag),
            }
        }
        "server_links" => Kind::ServerLinks {
            exit: exit_action(tag, ctx),
            columns: columns(tag),
            button_width: width(tag, "button_width", DEFAULT_BUTTON_WIDTH),
        },
        other => {
            crate::log_warn!("dialog", "unrecognized dialog type {other}");
            return None;
        }
    };

    Some(Arc::new(Dialog {
        title,
        external_title,
        can_close_with_escape: boolean(tag, "can_close_with_escape", true),
        pause: boolean(tag, "pause", true),
        after_action: tag
            .string("after_action")
            .map(|s| AfterAction::parse(&s.to_string()))
            .unwrap_or_default(),
        body: bodies(tag.get("body")),
        inputs: inputs(tag.get("inputs")),
        kind,
    }))
}

const DEFAULT_BUTTON_WIDTH: f32 = 150.0;

const DEFAULT_CONTENT_WIDTH: f32 = 200.0;

fn columns(tag: &NbtCompound) -> u32 {
    tag.int("columns").filter(|c| *c > 0).unwrap_or(2) as u32
}

fn exit_action(tag: &NbtCompound, ctx: &mut Ctx) -> Option<ActionButton> {
    action_button(tag.compound("exit_action")?, ctx)
}

fn width(tag: &NbtCompound, name: &str, default: f32) -> f32 {
    tag.int(name).unwrap_or(default as i32).clamp(1, 1024) as f32
}

fn boolean(tag: &NbtCompound, name: &str, default: bool) -> bool {
    tag.byte(name).map(|b| b != 0).unwrap_or(default)
}

fn strip_namespace(id: &str) -> &str {
    id.split_once(':').map(|(_, path)| path).unwrap_or(id)
}

fn component(tag: Option<&NbtTag>) -> Option<Vec<Span>> {
    let text = crate::client::chat_text::from_nbt_tag(tag?)?;
    Some(crate::client::chat_text::to_spans(&text))
}

fn compound_list(tag: Option<&NbtTag>) -> Vec<NbtCompound> {
    match tag {
        Some(NbtTag::List(NbtList::Compound(entries))) => entries.clone(),
        Some(NbtTag::Compound(entry)) => vec![entry.clone()],
        _ => Vec::new(),
    }
}

fn action_button(tag: &NbtCompound, ctx: &mut Ctx) -> Option<ActionButton> {
    Some(ActionButton {
        label: component(tag.get("label"))?,
        tooltip: component(tag.get("tooltip")),
        width: width(tag, "width", DEFAULT_BUTTON_WIDTH),
        action: tag.compound("action").and_then(|a| action(a, ctx)),
    })
}

fn action(tag: &NbtCompound, ctx: &mut Ctx) -> Option<Action> {
    let id = tag.string("type").map(|s| s.to_string())?;
    Some(match strip_namespace(&id) {
        "dynamic/run_command" => {
            Action::CommandTemplate(Template::parse(&tag.string("template")?.to_string())?)
        }
        "dynamic/custom" => Action::Custom {
            id: tag.string("id")?.to_string(),
            additions: tag.compound("additions").cloned().unwrap_or_default(),
        },
        _ => Action::Static(click(&id, tag, ctx)?),
    })
}

fn click(id: &str, tag: &NbtCompound, ctx: &mut Ctx) -> Option<Click> {
    Some(match strip_namespace(id) {
        "open_url" => Click::OpenUrl(tag.string("url")?.to_string()),
        "run_command" => Click::RunCommand(tag.string("command")?.to_string()),
        "suggest_command" => Click::SuggestCommand(tag.string("command")?.to_string()),
        "change_page" => Click::ChangePage(tag.int("page")?),
        "copy_to_clipboard" => Click::CopyToClipboard(tag.string("value")?.to_string()),
        "show_dialog" => {
            ctx.depth += 1;
            let dialog = sub_dialog(tag.get("dialog")?, ctx);
            ctx.depth -= 1;
            Click::ShowDialog(dialog?)
        }
        "custom" => Click::Custom {
            id: tag.string("id")?.to_string(),
            payload: tag.get("payload").cloned(),
        },
        other => {
            crate::log_warn!("dialog", "unrecognized dialog action {other}");
            return None;
        }
    })
}

fn sub_dialog(tag: &NbtTag, ctx: &mut Ctx) -> Option<Arc<Dialog>> {
    match tag {
        NbtTag::String(id) => {
            let entry = ctx.registry_entry_by_name(&id.to_string())?.clone();
            parse(&entry, ctx)
        }
        NbtTag::Compound(inline) => parse(inline, ctx),
        _ => None,
    }
}

fn dialog_list_buttons(tag: &NbtTag, button_width: f32, ctx: &mut Ctx) -> Vec<ActionButton> {
    ctx.depth += 1;
    let dialogs: Vec<Arc<Dialog>> = match tag {
        NbtTag::String(name) if name.to_string().starts_with('#') => {
            let name = name.to_string();
            let ids = tags()
                .lock()
                .unwrap()
                .get(&tag_key(&name))
                .cloned()
                .unwrap_or_default();
            ids.into_iter()
                .filter_map(|id| {
                    let entry = ctx.registry_entry(id)?.clone();
                    parse(&entry, ctx)
                })
                .collect()
        }
        NbtTag::List(NbtList::String(ids)) => {
            let ids: Vec<String> = ids.iter().map(|id| id.to_string()).collect();
            ids.iter()
                .filter_map(|id| {
                    let entry = ctx.registry_entry_by_name(id)?.clone();
                    parse(&entry, ctx)
                })
                .collect()
        }
        NbtTag::List(NbtList::Compound(entries)) => entries
            .iter()
            .filter_map(|entry| parse(entry, ctx))
            .collect(),
        single => sub_dialog(single, ctx).into_iter().collect(),
    };
    ctx.depth -= 1;

    dialogs
        .into_iter()
        .map(|dialog| ActionButton {
            label: dialog.external_title.clone(),
            tooltip: None,
            width: button_width,
            action: Some(Action::Static(Click::ShowDialog(dialog))),
        })
        .collect()
}

fn tag_key(name: &str) -> String {
    Identifier::new(name.strip_prefix('#').unwrap_or(name)).to_string()
}

fn bodies(tag: Option<&NbtTag>) -> Vec<Body> {
    compound_list(tag).iter().filter_map(body).collect()
}

fn body(tag: &NbtCompound) -> Option<Body> {
    let id = tag.string("type").map(|s| s.to_string())?;
    Some(match strip_namespace(&id) {
        "plain_message" => Body::Plain {
            contents: component(tag.get("contents"))?,
            width: width(tag, "width", DEFAULT_CONTENT_WIDTH),
        },
        "item" => {
            let item = tag.compound("item")?;
            Body::Item {
                stack: crate::play::inventory_bridge::slot_stack_nbt(item)?,
                description: plain_message(tag.get("description")),
                show_decorations: boolean(tag, "show_decorations", true),
                show_tooltip: boolean(tag, "show_tooltip", true),
                width: tag.int("width").unwrap_or(16).clamp(1, 256) as f32,
                height: tag.int("height").unwrap_or(16).clamp(1, 256) as f32,
            }
        }
        other => {
            crate::log_warn!("dialog", "unrecognized dialog body {other}");
            return None;
        }
    })
}

fn plain_message(tag: Option<&NbtTag>) -> Option<(Vec<Span>, f32)> {
    match tag? {
        NbtTag::Compound(c) if c.contains("contents") => Some((
            component(c.get("contents"))?,
            width(c, "width", DEFAULT_CONTENT_WIDTH),
        )),
        other => Some((component(Some(other))?, DEFAULT_CONTENT_WIDTH)),
    }
}

fn inputs(tag: Option<&NbtTag>) -> Vec<Input> {
    compound_list(tag).iter().filter_map(input).collect()
}

fn input(tag: &NbtCompound) -> Option<Input> {
    let key = tag.string("key")?.to_string();
    if !is_valid_variable_name(&key) {
        crate::log_warn!("dialog", "input key {key} is not a valid macro variable");
        return None;
    }
    Some(Input {
        key,
        control: control(tag)?,
    })
}

fn control(tag: &NbtCompound) -> Option<Control> {
    let id = tag.string("type").map(|s| s.to_string())?;
    let label = || component(tag.get("label")).unwrap_or_default();
    Some(match strip_namespace(&id) {
        "text" => Control::Text {
            width: width(tag, "width", DEFAULT_CONTENT_WIDTH),
            label: label(),
            label_visible: boolean(tag, "label_visible", true),
            initial: tag
                .string("initial")
                .map(|s| s.to_string())
                .unwrap_or_default(),
            max_length: tag.int("max_length").filter(|n| *n > 0).unwrap_or(32) as usize,
            multiline: tag.compound("multiline").map(|m| {
                let max_lines = m.int("max_lines").filter(|n| *n > 0).map(|n| n as usize);
                Multiline {
                    max_lines,
                    height: m
                        .int("height")
                        .map(|h| h.clamp(1, 512))
                        .unwrap_or_else(|| (9 * max_lines.unwrap_or(4) as i32 + 8).min(512))
                        as f32,
                }
            }),
        },
        "single_option" => {
            let mut options = Vec::new();
            let mut initial = 0;
            for (i, entry) in choices(tag.get("options")).into_iter().enumerate() {
                if entry.1 {
                    initial = i;
                }
                options.push(entry.0);
            }
            if options.is_empty() {
                return None;
            }
            Control::SingleOption {
                width: width(tag, "width", DEFAULT_CONTENT_WIDTH),
                label: label(),
                label_visible: boolean(tag, "label_visible", true),
                options,
                initial,
            }
        }
        "boolean" => Control::Boolean {
            label: label(),
            initial: boolean(tag, "initial", false),
            on_true: tag
                .string("on_true")
                .map(|s| s.to_string())
                .unwrap_or_else(|| "true".to_string()),
            on_false: tag
                .string("on_false")
                .map(|s| s.to_string())
                .unwrap_or_else(|| "false".to_string()),
        },
        "number_range" => Control::NumberRange {
            width: width(tag, "width", DEFAULT_CONTENT_WIDTH),
            label: label(),
            label_format: tag
                .string("label_format")
                .map(|s| s.to_string())
                .unwrap_or_else(|| "options.generic_value".to_string()),
            range: RangeInfo {
                start: tag.float("start")?,
                end: tag.float("end")?,
                initial: tag.float("initial"),
                step: tag.float("step").filter(|s| *s > 0.0),
            },
        },
        other => {
            crate::log_warn!("dialog", "unrecognized dialog input {other}");
            return None;
        }
    })
}

fn choices(tag: Option<&NbtTag>) -> Vec<(Choice, bool)> {
    let entries: Vec<NbtTag> = match tag {
        Some(NbtTag::List(list)) => list.as_nbt_tags(),
        Some(single) => vec![single.clone()],
        None => Vec::new(),
    };
    entries
        .into_iter()
        .filter_map(|entry| match entry {
            NbtTag::String(id) => {
                let id = id.to_string();
                Some((
                    Choice {
                        display: literal(&id),
                        id,
                    },
                    false,
                ))
            }
            NbtTag::Compound(c) => {
                let id = c.string("id")?.to_string();
                Some((
                    Choice {
                        display: component(c.get("display")).unwrap_or_else(|| literal(&id)),
                        id,
                    },
                    boolean(&c, "initial", false),
                ))
            }
            _ => None,
        })
        .collect()
}

fn literal(text: &str) -> Vec<Span> {
    crate::text::styled(text, crate::text::Style::default())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn template_parses_like_vanilla() {
        let t = Template::parse("/say $(name) and $(other)!").unwrap();
        assert_eq!(t.variables, vec!["name", "other"]);
        assert_eq!(t.segments, vec!["/say ", " and ", "!"]);

        assert!(Template::parse("/say hello").is_none());
        assert!(Template::parse("/say $(name").is_none());
        assert!(Template::parse("/say $(na-me)").is_none());
        assert!(Template::parse("costs $5").is_none());
        assert_eq!(
            Template::parse("costs $5 for $(who)")
                .unwrap()
                .instantiate(&HashMap::from([("who".into(), "you".into())])),
            "costs $5 for you"
        );
    }

    #[test]
    fn missing_arguments_substitute_empty() {
        let t = Template::parse("/give $(who) $(what)").unwrap();
        let args = HashMap::from([("who".to_string(), "steve".to_string())]);
        assert_eq!(t.instantiate(&args), "/give steve ");
    }

    #[test]
    fn text_is_escaped_for_commands_but_not_for_nbt() {
        let value = Value::Text("say \"hi\"\n".to_string());
        assert_eq!(value.substitution(), "say \\\"hi\\\"\\n");
        assert_eq!(value.tag(), NbtTag::String("say \"hi\"\n".into()));
    }

    #[test]
    fn booleans_disagree_between_the_two_forms() {
        let value = Value::Boolean {
            selected: true,
            on_true: "yes".to_string(),
            on_false: "no".to_string(),
        };
        assert_eq!(value.substitution(), "yes");
        assert_eq!(value.tag(), NbtTag::Byte(1));
    }

    #[test]
    fn slider_steps_are_anchored_on_the_initial_value() {
        let range = RangeInfo {
            start: 0.0,
            end: 10.0,
            initial: Some(2.5),
            step: Some(2.0),
        };
        assert_eq!(range.initial_slider(), 0.25);
        assert_eq!(range.scaled(0.25), 2.5);
        assert_eq!(range.scaled(0.45), 4.5);
        assert_eq!(range.scaled(1.0), 8.5);
    }

    #[test]
    fn slider_without_a_step_is_a_lerp() {
        let range = RangeInfo {
            start: 0.0,
            end: 4.0,
            initial: None,
            step: None,
        };
        assert_eq!(range.initial_slider(), 0.5);
        assert_eq!(range.scaled(0.5), 2.0);
        assert_eq!(RangeInfo::value_string(range.scaled(0.5)), "2");
        assert_eq!(RangeInfo::value_string(range.scaled(0.125)), "0.5");
    }

    #[test]
    fn degenerate_range_is_pinned_to_the_middle() {
        let range = RangeInfo {
            start: 3.0,
            end: 3.0,
            initial: None,
            step: None,
        };
        assert_eq!(range.initial_slider(), 0.5);
        assert_eq!(range.scaled(0.7), 3.0);
    }

    #[test]
    fn custom_actions_carry_every_input() {
        let mut additions = NbtCompound::new();
        additions.insert("kept", NbtTag::Int(7));
        additions.insert("overwritten", NbtTag::Int(1));
        let action = Action::Custom {
            id: "test:submit".to_string(),
            additions,
        };
        let values = HashMap::from([
            ("overwritten".to_string(), Value::Number(2.5)),
            ("name".to_string(), Value::Text("steve".to_string())),
        ]);
        let Some(Click::Custom {
            id,
            payload: Some(NbtTag::Compound(payload)),
        }) = action.resolve(&values)
        else {
            panic!("expected a custom click event");
        };
        assert_eq!(id, "test:submit");
        assert_eq!(payload.int("kept"), Some(7));
        assert_eq!(payload.float("overwritten"), Some(2.5));
        assert_eq!(
            payload.string("name").map(|s| s.to_string()).as_deref(),
            Some("steve")
        );
    }
}
