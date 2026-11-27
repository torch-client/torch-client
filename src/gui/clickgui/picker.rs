use crate::gui::widgets::TextBox;
use crate::modules::list::{BitList, Match};

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
        let query = TextBox::pinned(32, "");
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
        for g in 0..self.list.group_count() {
            let range = self.list.group_range(g);
            let mut any = query.is_empty();
            let start = self.rows.len();
            self.rows.push(Row::Header(g as u16));
            if query.is_empty() {
                if self.is_open(g) {
                    self.rows.extend(range.map(|i| Row::Item(i as u32)));
                }
            } else {
                for kind in Match::ALL {
                    for i in range.clone() {
                        if self.list.row_match(i, query) == Some(kind) {
                            self.rows.push(Row::Item(i as u32));
                            any = true;
                        }
                    }
                }
            }
            if !any && !self.list.group_matches(g, query) {
                self.rows.truncate(start);
            }
        }
    }
}
