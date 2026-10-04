use crate::gui::widgets::TextBox;
use crate::modules::list::BitList;

#[derive(Clone, Copy, PartialEq)]
pub enum Row {
    Header(u16),
    Item(u32),
}

pub struct Picker {
    pub list: &'static BitList,
    pub query: TextBox,
    pub open: u32,
    pub channels: Option<u32>,
    pub rows: Vec<Row>,
    pub built: Option<(u32, u32, usize)>,
}

impl Picker {
    pub fn new(list: &'static BitList) -> Picker {
        let mut query = TextBox::new(32, "Search");
        query.browser_keyboard = false;
        query.focused = true;
        Picker {
            list,
            query,
            open: 0,
            channels: None,
            rows: Vec::with_capacity(64),
            built: None,
        }
    }

    pub fn is_open(&self, g: usize) -> bool {
        self.open >> g & 1 != 0
    }

    pub fn toggle_group(&mut self, g: usize) {
        self.open ^= 1 << g;
    }

    pub fn rebuild(&mut self, dirty: bool) {
        let query = self.query.text.as_str();
        let key = (self.open, self.list.generation(), query.len());
        if !dirty && self.built == Some(key) {
            return;
        }
        self.built = Some(key);
        self.rows.clear();
        for g in 0..self.list.groups.len() {
            let range = self.list.group_range(g);
            let mut any = query.is_empty();
            let start = self.rows.len();
            self.rows.push(Row::Header(g as u16));
            if self.is_open(g) || !query.is_empty() {
                for i in range {
                    if query.is_empty() || contains_ci(self.list.label(i), query) {
                        self.rows.push(Row::Item(i as u32));
                        any = true;
                    }
                }
            }
            if !any && !contains_ci(self.list.groups[g], query) {
                self.rows.truncate(start);
            }
        }
    }
}

pub fn contains_ci(hay: &str, needle: &str) -> bool {
    let (h, n) = (hay.as_bytes(), needle.as_bytes());
    if n.is_empty() {
        return true;
    }
    if n.len() > h.len() {
        return false;
    }
    (0..=h.len() - n.len()).any(|i| {
        n.iter()
            .zip(&h[i..])
            .all(|(a, b)| a.eq_ignore_ascii_case(b))
    })
}
