use crate::gui::painter::Painter;
use crate::gui::{Screen, ScreenCtx};
use crate::session::SharedMutex;
use crate::text::{Span, Style, texts};

const SECTION_HEADING: &str = "============";
const NAME_PREFIX: &str = "           ";
const OBFUSCATE_TOKEN: &str = "\u{a7}f\u{a7}k\u{a7}a\u{a7}b";
const OBFUSCATE_SEED: i64 = 8124371;
const POEM_WRAP_W: f32 = 256.0;
const LINE_PITCH: f32 = 12.0;
const SCROLL_SPEED: f32 = 0.5;
const SPEEDUP: f32 = 5.0;
const SPEEDUP_FAST: f32 = 15.0;

const YELLOW: u32 = 0xFFFF55;
const GRAY: u32 = 0xAAAAAA;
const WHITE: u32 = 0xFFFFFF;

struct Line {
    spans: Vec<Span>,
    centered: bool,
}

#[derive(Default)]
pub struct WinState {
    lines: Vec<Line>,
    built: bool,
    scroll: f32,
    last_time: Option<f32>,
    skip: bool,
}

impl WinState {
    pub fn open(&mut self) {
        *self = WinState::default();
    }

    pub fn skip(&mut self) {
        self.skip = true;
    }

    fn empty(&mut self) {
        self.lines.push(Line {
            spans: Vec::new(),
            centered: false,
        });
    }

    fn credit(&mut self, text: &str, color: u32, centered: bool) {
        self.lines.push(Line {
            spans: crate::text::styled(text, Style::colored(color)),
            centered,
        });
    }

    fn add_poem(&mut self, text: &str, name: &str, font: &crate::text::Font) {
        let mut random = crate::util::javarandom::JavaRandom::new(OBFUSCATE_SEED);
        for line in text.lines() {
            let mut line = line.replace("PLAYERNAME", name);
            while let Some(pos) = line.find(OBFUSCATE_TOKEN) {
                let n = random.next_int(4) as usize + 3;
                line = format!(
                    "{}\u{a7}f\u{a7}k{}{}",
                    &line[..pos],
                    &"XXXXXXXX"[..n],
                    &line[pos + OBFUSCATE_TOKEN.len()..]
                );
            }
            let spans = crate::text::parse_formatted(&line);
            if !spans.is_empty() {
                for wrapped in font.wrap(&spans, POEM_WRAP_W) {
                    self.lines.push(Line {
                        spans: wrapped,
                        centered: false,
                    });
                }
            }
            self.empty();
        }
        for _ in 0..8 {
            self.empty();
        }
    }

    fn add_credits(&mut self) {
        for section in texts::credits().iter() {
            self.credit(SECTION_HEADING, WHITE, true);
            self.credit(&section.section, YELLOW, true);
            self.credit(SECTION_HEADING, WHITE, true);
            self.empty();
            self.empty();
            for discipline in &section.disciplines {
                if !discipline.discipline.is_empty() {
                    self.credit(&discipline.discipline, YELLOW, true);
                    self.empty();
                    self.empty();
                }
                for title in &discipline.titles {
                    self.credit(&title.title, GRAY, false);
                    for name in &title.names {
                        self.credit(&format!("{NAME_PREFIX}{name}"), WHITE, false);
                    }
                    self.empty();
                    self.empty();
                }
            }
        }
    }

    fn build(&mut self, font: &crate::text::Font) {
        let name = crate::client::bot::username();
        if let Some(poem) = texts::end_poem() {
            self.add_poem(&poem, &name, font);
        }
        self.add_credits();
        if let Some(post) = texts::post_credits() {
            self.add_poem(&post, &name, font);
        }
        self.built = true;
    }
}

pub fn draw(
    p: &mut Painter,
    st: &mut WinState,
    ctx: &ScreenCtx,
    shared: &SharedMutex,
) -> Option<Screen> {
    if !st.built {
        st.build(&p.atlas.font);
    }
    let (vw, vh) = (ctx.vw.floor(), ctx.vh.floor());

    let mut speed = SCROLL_SPEED;
    if ctx.input.space_down {
        speed *= SPEEDUP + if ctx.input.ctrl { SPEEDUP_FAST } else { 0.0 };
    }
    if ctx.input.up_down {
        speed = -speed;
    }
    let dt = st.last_time.map_or(0.0, |t| (ctx.input.time - t).max(0.0));
    st.last_time = Some(ctx.input.time);
    st.scroll = (st.scroll + dt * 20.0 * speed).max(0.0);

    let total = st.lines.len() as f32 * LINE_PITCH;
    if ctx.input.escape || st.skip || st.scroll > total + vh + vh + 24.0 {
        shared.lock().unwrap().respawn_requested = true;
        return Some(Screen::None);
    }

    p.fill(0.0, 0.0, ctx.vw, ctx.vh, 0xFF00_0000);

    let scale = ctx.input.scale.max(1.0);
    let snap = |y: f32| (y * scale).round() / scale;
    let y_offs = -st.scroll;
    let logo_x = (vw / 2.0).floor() - 128.0;
    let logo_y = vh + 50.0;
    crate::gui::title::logo(p, vw, snap(logo_y + y_offs));

    let last = st.lines.len().saturating_sub(1);
    let mut pin = 0.0;
    let mut y = logo_y + 100.0;
    for (i, line) in st.lines.iter().enumerate() {
        if i == last {
            let diff = y + y_offs - (vh / 2.0).floor() + 6.0;
            if diff < 0.0 {
                pin = -diff;
            }
        }
        if y + y_offs + LINE_PITCH + 8.0 > 0.0 && y + y_offs < vh && !line.spans.is_empty() {
            let draw_y = snap(y + y_offs + pin);
            let x = if line.centered {
                logo_x + 128.0 - (p.atlas.font.width(&line.spans) / 2.0).floor()
            } else {
                logo_x
            };
            p.text(&line.spans, x, draw_y, true);
        }
        y += LINE_PITCH;
    }
    None
}
