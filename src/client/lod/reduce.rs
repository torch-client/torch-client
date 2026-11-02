use super::render_point as rp;

const SPECIAL_CASES: i32 = 2;
const NULL: u16 = u16::MAX;
const DEFAULT_LINKS: [u16; 4] = [NULL; 4];

const LOWER: usize = 0;
const HIGHER: usize = 1;
const SMALLER: usize = 2;
const BIGGER: usize = 3;

#[inline]
fn is_data_visible(p: u64) -> bool {
    is_alpha_visible(rp::alpha(p))
}

#[inline]
fn is_alpha_visible(alpha: u8) -> bool {
    alpha >= 16
}

#[inline]
fn size_of(p: u64) -> i32 {
    rp::y_max(p) - rp::y_min(p)
}

pub struct ReducingList {
    lowest: u16,
    highest: u16,
    smallest: u16,
    biggest: u16,
    size_with_air: i32,
    size_without_air: i32,
    links: Vec<[u16; 4]>,
    data: Vec<u64>,
    sorting: Vec<u16>,
}

impl Default for ReducingList {
    fn default() -> Self {
        Self::new()
    }
}

impl ReducingList {
    pub fn new() -> Self {
        Self {
            lowest: NULL,
            highest: NULL,
            smallest: NULL,
            biggest: NULL,
            size_with_air: 0,
            size_without_air: 0,
            links: Vec::new(),
            data: Vec::new(),
            sorting: Vec::new(),
        }
    }

    #[inline]
    fn link(&self, i: u16, which: usize) -> u16 {
        self.links[i as usize][which]
    }

    #[inline]
    fn set_link(&mut self, i: u16, which: usize, to: u16) {
        self.links[i as usize][which] = to;
    }

    #[inline]
    fn lower(&self, i: u16) -> u16 {
        self.link(i, LOWER)
    }

    #[inline]
    fn higher(&self, i: u16) -> u16 {
        self.link(i, HIGHER)
    }

    #[inline]
    fn smaller(&self, i: u16) -> u16 {
        self.link(i, SMALLER)
    }

    #[inline]
    fn bigger(&self, i: u16) -> u16 {
        self.link(i, BIGGER)
    }

    #[inline]
    fn set_lower(&mut self, i: u16, to: u16) {
        self.set_link(i, LOWER, to);
    }

    #[inline]
    fn set_higher(&mut self, i: u16, to: u16) {
        self.set_link(i, HIGHER, to);
    }

    #[inline]
    fn set_smaller(&mut self, i: u16, to: u16) {
        self.set_link(i, SMALLER, to);
    }

    #[inline]
    fn set_bigger(&mut self, i: u16, to: u16) {
        self.set_link(i, BIGGER, to);
    }

    #[inline]
    fn data_at(&self, i: u16) -> u64 {
        self.data[i as usize]
    }

    #[inline]
    fn min_y(&self, i: u16) -> i32 {
        rp::y_min(self.data_at(i))
    }

    #[inline]
    fn size(&self, i: u16) -> i32 {
        size_of(self.data_at(i))
    }

    #[inline]
    fn alpha(&self, i: u16) -> u8 {
        rp::alpha(self.data_at(i))
    }

    #[inline]
    fn set_min_y(&mut self, i: u16, min_y: i32) {
        let p = self.data[i as usize];
        self.data[i as usize] = rp::set_y_min(p, min_y);
    }

    #[inline]
    fn is_index_visible(&self, i: u16) -> bool {
        is_data_visible(self.data_at(i))
    }

    fn clear_to_empty(&mut self) {
        self.lowest = NULL;
        self.highest = NULL;
        self.smallest = NULL;
        self.biggest = NULL;
        self.size_with_air = 0;
        self.size_without_air = 0;
    }

    pub fn populate(&mut self, view: &[u64]) -> bool {
        let size = view.len();
        self.clear_to_empty();
        if size == 0 {
            self.links.clear();
            self.data.clear();
            self.sorting.clear();
            return true;
        }

        let capacity = (size << 1) - 1;
        self.sorting.clear();
        self.sorting.resize(capacity, 0);
        self.links.clear();
        self.links.resize(capacity, DEFAULT_LINKS);
        self.data.clear();
        self.data.resize(capacity, 0);

        let mut size_without_air = 0usize;
        for &p in view {
            if is_data_visible(p) && rp::y_min(p) < rp::y_max(p) {
                self.data[size_without_air] = p;
                self.sorting[size_without_air] = size_without_air as u16;
                size_without_air += 1;
            }
        }
        if size_without_air == 0 {
            return true;
        }

        self.sort_by_position(size_without_air);
        let mut size_with_air = size_without_air;
        for s in 1..size_without_air {
            let lower = self.sorting[s - 1];
            let higher = self.sorting[s];
            let lower_data = self.data_at(lower);
            let higher_data = self.data_at(higher);
            let lower_max_y = rp::y_max(lower_data);
            let higher_min_y = rp::y_min(higher_data);
            if lower_max_y == higher_min_y {
                self.set_higher(lower, higher);
                self.set_lower(higher, lower);
            } else if lower_max_y < higher_min_y {
                let air = size_with_air as u16;
                self.data[size_with_air] = rp::create(
                    higher_min_y,
                    lower_max_y,
                    0,
                    rp::sky_light(higher_data),
                    rp::block_light(higher_data),
                    rp::material(higher_data),
                );
                self.sorting[size_with_air] = air;
                self.set_lower(higher, air);
                self.set_higher(lower, air);
                self.set_lower(air, lower);
                self.set_higher(air, higher);
                size_with_air += 1;
            } else {
                self.clear_to_empty();
                return false;
            }
        }
        self.lowest = self.sorting[0];
        self.highest = self.sorting[size_without_air - 1];

        self.sort_by_size(size_with_air);
        for s in 1..size_with_air {
            let smaller = self.sorting[s - 1];
            let bigger = self.sorting[s];
            self.set_bigger(smaller, bigger);
            self.set_smaller(bigger, smaller);
        }
        self.smallest = self.sorting[0];
        self.biggest = self.sorting[size_with_air - 1];

        self.size_with_air = size_with_air as i32;
        self.size_without_air = size_without_air as i32;
        true
    }

    pub fn reduce(&mut self, target: i32) {
        if self.merge_very_small_connected_segments(target) {
            return;
        }
        if self.merge_connected_segments(target) {
            return;
        }
        if self.remove_least_important_segments(target) {
            return;
        }
        self.force_bottom_to_merge(target);
    }

    fn remove(&mut self, i: u16) {
        let [lower, higher, smaller, bigger] = self.links[i as usize];
        let alpha = self.alpha(i);

        if lower != NULL {
            self.set_higher(lower, higher);
        } else {
            self.lowest = higher;
        }
        if higher != NULL {
            self.set_lower(higher, lower);
        } else {
            self.highest = lower;
        }
        if smaller != NULL {
            self.set_bigger(smaller, bigger);
        } else {
            self.smallest = bigger;
        }
        if bigger != NULL {
            self.set_smaller(bigger, smaller);
        } else {
            self.biggest = smaller;
        }

        self.data[i as usize] = 0;
        self.links[i as usize] = DEFAULT_LINKS;
        self.size_with_air -= 1;
        if is_alpha_visible(alpha) {
            self.size_without_air -= 1;
        }
    }

    fn sort_by_size_and_relink(&mut self) {
        if self.size_with_air <= 1 {
            return;
        }
        let mut write = 0usize;
        let mut read = self.lowest;
        while read != NULL {
            if self.data_at(read) != 0 {
                self.sorting[write] = read;
                write += 1;
            }
            read = self.higher(read);
        }

        self.sort_by_size(write);
        for k in 1..write {
            let smaller = self.sorting[k - 1];
            let bigger = self.sorting[k];
            self.set_smaller(bigger, smaller);
            self.set_bigger(smaller, bigger);
        }
        if write == 0 {
            write = 1;
        }
        self.smallest = self.sorting[0];
        self.biggest = self.sorting[write - 1];
        self.set_smaller(self.smallest, NULL);
        self.set_bigger(self.biggest, NULL);
    }

    fn sort_by_size(&mut self, n: usize) {
        let data = &self.data;
        quick_sort(&mut self.sorting[..n], |i| size_of(data[i as usize]));
    }

    fn sort_by_position(&mut self, n: usize) {
        let data = &self.data;
        quick_sort(&mut self.sorting[..n], |i| rp::y_min(data[i as usize]));
    }

    fn resort_size(&mut self, smaller: u16) {
        let mut bigger = self.bigger(smaller);
        if bigger == NULL || self.size(smaller) <= self.size(bigger) {
            return;
        }

        let smallest = self.smaller(smaller);
        if smallest != NULL {
            self.set_bigger(smallest, bigger);
        } else {
            self.smallest = bigger;
        }
        self.set_smaller(bigger, smallest);

        loop {
            bigger = self.bigger(bigger);
            if !(bigger != NULL && self.size(smaller) > self.size(bigger)) {
                break;
            }
        }

        let before = if bigger != NULL {
            self.smaller(bigger)
        } else {
            self.biggest
        };
        self.set_smaller(smaller, before);
        self.set_bigger(smaller, bigger);
        if bigger != NULL {
            self.set_smaller(bigger, smaller);
        } else {
            self.biggest = smaller;
        }
        let smallest = self.smaller(smaller);
        if smallest != NULL {
            self.set_bigger(smallest, smaller);
        } else {
            self.smallest = smaller;
        }
    }

    fn try_merge_step1(&mut self, current: u16, fast_path: bool) -> u16 {
        let mut result = if fast_path {
            self.smaller(current)
        } else {
            self.bigger(current)
        };
        let higher = self.higher(current);
        let lower = self.lower(current);
        let alpha = self.alpha(current);

        let (to_extend_downwards, to_remove);
        if higher != NULL && self.alpha(higher) == alpha {
            if lower != NULL && self.alpha(lower) == alpha {
                if self.size(higher) <= self.size(lower) {
                    to_extend_downwards = higher;
                    to_remove = current;
                } else {
                    to_extend_downwards = current;
                    to_remove = lower;
                }
            } else {
                to_extend_downwards = higher;
                to_remove = current;
            }
        } else if lower != NULL && self.alpha(lower) == alpha {
            to_extend_downwards = current;
            to_remove = lower;
        } else {
            return result;
        }

        if result == to_remove {
            result = self.smaller(result);
        }

        let min_y = self.min_y(to_remove);
        self.set_min_y(to_extend_downwards, min_y);
        if !fast_path {
            self.resort_size(to_extend_downwards);
        }
        self.remove(to_remove);

        if fast_path { result } else { self.smallest }
    }

    fn lower_node(&self, size: i32) -> u16 {
        let mut node = self.smallest;
        while node != NULL {
            if self.size(node) >= size {
                return self.smaller(node);
            }
            node = self.bigger(node);
        }
        self.biggest
    }

    fn merge_very_small_connected_segments(&mut self, target: i32) -> bool {
        for special in 1..=SPECIAL_CASES {
            let mut current = self.lower_node(special + 1);
            while current != NULL {
                if self.size_without_air <= target {
                    self.sort_by_size_and_relink();
                    return true;
                }
                current = self.try_merge_step1(current, true);
            }
            self.sort_by_size_and_relink();
        }
        false
    }

    fn merge_connected_segments(&mut self, target: i32) -> bool {
        let mut current = self.smallest;
        while current != NULL {
            if self.size_without_air <= target {
                return true;
            }
            current = self.try_merge_step1(current, false);
        }
        false
    }

    fn remove_least_important_segments(&mut self, target: i32) -> bool {
        let mut center = self.smallest;
        while center != NULL {
            if self.size_without_air <= target {
                return true;
            }
            let lower = self.lower(center);
            let higher = self.higher(center);
            if lower != NULL && higher != NULL && self.alpha(lower) == self.alpha(higher) {
                let min_y = self.min_y(lower);
                self.set_min_y(higher, min_y);
                self.resort_size(higher);
                self.remove(lower);
                self.remove(center);
                center = self.smallest;
            } else {
                center = self.bigger(center);
            }
        }
        false
    }

    fn force_bottom_to_merge(&mut self, target: i32) {
        let mut lowest = self.lowest;
        while lowest != NULL {
            if self.size_without_air <= target {
                return;
            }
            let low_y = self.min_y(lowest);
            let mut higher = self.higher(lowest);
            loop {
                if higher == NULL {
                    self.clear_to_empty();
                    return;
                }
                if self.is_index_visible(higher) {
                    self.set_min_y(higher, low_y);
                    self.resort_size(higher);
                    self.remove(lowest);
                    lowest = self.lowest;
                    break;
                }
                self.remove(lowest);
                lowest = higher;
                higher = self.higher(higher);
            }
        }
    }

    pub fn copy_to(&self, out: &mut [u64]) {
        let mut write = 0usize;
        let mut node = self.highest;
        while node != NULL {
            if self.is_index_visible(node) {
                out[write] = self.data_at(node);
                write += 1;
            }
            node = self.lower(node);
        }
        let len = out.len();
        out[write.min(len)..].fill(rp::EMPTY);
    }
}

pub fn reduce_to_one(view: &[u64]) -> u64 {
    let Some(first) = view.iter().position(|&p| is_data_visible(p)) else {
        return rp::EMPTY;
    };
    let mut highest = view[first];
    let mut lowest = view[first];
    for &p in &view[first..] {
        if is_data_visible(p) {
            if rp::y_max(p) > rp::y_max(highest) {
                highest = p;
            } else if rp::y_min(p) < rp::y_min(lowest) {
                lowest = p;
            }
        }
    }
    rp::set_y_min(highest, rp::y_min(lowest))
}

fn quick_sort(s: &mut [u16], key: impl Fn(u16) -> i32 + Copy) {
    quick_sort_range(s, 0, s.len(), key);
}

fn quick_sort_range(s: &mut [u16], from: usize, to: usize, key: impl Fn(u16) -> i32 + Copy) {
    let cmp = |s: &[u16], a: usize, b: usize| key(s[a]).cmp(&key(s[b])) as i32;
    let len = to - from;
    if len < 16 {
        for i in from..to {
            let mut j = i;
            while j > from && cmp(s, j - 1, j) > 0 {
                s.swap(j, j - 1);
                j -= 1;
            }
        }
        return;
    }

    let mut m = from + len / 2;
    let mut l = from;
    let mut n = to - 1;
    if len > 128 {
        let step = len / 8;
        l = med3(s, l, l + step, l + 2 * step, cmp);
        m = med3(s, m - step, m, m + step, cmp);
        n = med3(s, n - 2 * step, n - step, n, cmp);
    }
    m = med3(s, l, m, n, cmp);

    let (from_i, to_i) = (from as isize, to as isize);
    let mut a = from_i;
    let mut b = a;
    let mut c = to_i - 1;
    let mut d = c;
    let mut m = m as isize;
    loop {
        while b <= c {
            let comparison = cmp(s, b as usize, m as usize);
            if comparison > 0 {
                break;
            }
            if comparison == 0 {
                if a == m {
                    m = b;
                } else if b == m {
                    m = a;
                }
                s.swap(a as usize, b as usize);
                a += 1;
            }
            b += 1;
        }
        while c >= b {
            let comparison = cmp(s, c as usize, m as usize);
            if comparison < 0 {
                break;
            }
            if comparison == 0 {
                if c == m {
                    m = d;
                } else if d == m {
                    m = c;
                }
                s.swap(c as usize, d as usize);
                d -= 1;
            }
            c -= 1;
        }
        if b > c {
            break;
        }
        if b == m {
            m = d;
        } else if c == m {
            m = c;
        }
        s.swap(b as usize, c as usize);
        b += 1;
        c -= 1;
    }

    let block_swap = |s: &mut [u16], mut x: isize, mut y: isize, count: isize| {
        for _ in 0..count {
            s.swap(x as usize, y as usize);
            x += 1;
            y += 1;
        }
    };
    let k = (a - from_i).min(b - a);
    block_swap(s, from_i, b - k, k);
    let k = (d - c).min(to_i - d - 1);
    block_swap(s, b, to_i - k, k);

    let k = b - a;
    if k > 1 {
        quick_sort_range(s, from, from + k as usize, key);
    }
    let k = d - c;
    if k > 1 {
        quick_sort_range(s, to - k as usize, to, key);
    }
}

fn med3(
    s: &[u16],
    a: usize,
    b: usize,
    c: usize,
    cmp: impl Fn(&[u16], usize, usize) -> i32,
) -> usize {
    let ab = cmp(s, a, b);
    let ac = cmp(s, a, c);
    let bc = cmp(s, b, c);
    if ab < 0 {
        if bc < 0 {
            b
        } else if ac < 0 {
            c
        } else {
            a
        }
    } else if bc > 0 {
        b
    } else if ac > 0 {
        c
    } else {
        a
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const OPAQUE: u32 = 0xFF00_0000;

    fn slab(max_y: i32, min_y: i32, rgb: u32) -> u64 {
        rp::create(max_y, min_y, OPAQUE | rgb, 15, 0, 2)
    }

    fn visible(out: &[u64]) -> Vec<u64> {
        out.iter().copied().filter(|&p| rp::exists(p)).collect()
    }

    #[test]
    fn quick_sort_matches_a_plain_sort() {
        let mut x = 12345u32;
        let keys: Vec<i32> = (0..300)
            .map(|_| {
                x = x.wrapping_mul(1_103_515_245).wrapping_add(12345);
                ((x >> 16) % 37) as i32
            })
            .collect();
        let mut idx: Vec<u16> = (0..keys.len() as u16).collect();
        quick_sort(&mut idx, |i| keys[i as usize]);
        let sorted: Vec<i32> = idx.iter().map(|&i| keys[i as usize]).collect();
        let mut expected = keys.clone();
        expected.sort_unstable();
        assert_eq!(sorted, expected);
        let mut seen = idx.clone();
        seen.sort_unstable();
        assert_eq!(seen, (0..keys.len() as u16).collect::<Vec<_>>());
    }

    #[test]
    fn reduce_to_one_spans_the_column() {
        let view = [
            slab(100, 90, 1),
            slab(90, 40, 2),
            slab(30, 10, 3),
            rp::EMPTY,
        ];
        let p = reduce_to_one(&view);
        assert_eq!(rp::y_max(p), 100);
        assert_eq!(rp::y_min(p), 10);
        assert_eq!(rp::blue(p), 1);
        assert_eq!(reduce_to_one(&[rp::EMPTY, rp::EMPTY]), rp::EMPTY);
    }

    #[test]
    fn touching_slabs_reduce_and_keep_their_extent() {
        let view = [
            slab(64, 63, 1),
            slab(63, 60, 2),
            slab(60, 59, 3),
            slab(59, 50, 4),
            slab(50, 0, 5),
        ];
        let mut list = ReducingList::new();
        assert!(list.populate(&view));
        list.reduce(2);
        let mut out = [0u64; 2];
        list.copy_to(&mut out);
        let v = visible(&out);
        assert!(!v.is_empty() && v.len() <= 2);
        assert_eq!(rp::y_max(v[0]), 64);
        assert_eq!(rp::y_min(*v.last().unwrap()), 0);
        for pair in v.windows(2) {
            assert_eq!(rp::y_min(pair[0]), rp::y_max(pair[1]));
        }
    }

    #[test]
    fn gaps_survive_a_reduction() {
        let view = [slab(60, 50, 1), slab(45, 40, 2), slab(40, 0, 3)];
        let mut list = ReducingList::new();
        assert!(list.populate(&view));
        list.reduce(2);
        let mut out = [0u64; 2];
        list.copy_to(&mut out);
        assert_eq!((rp::y_max(out[0]), rp::y_min(out[0])), (60, 50));
        assert_eq!((rp::y_max(out[1]), rp::y_min(out[1])), (45, 0));
        assert_eq!(rp::blue(out[1]), 2);
    }

    #[test]
    fn invisible_column_copies_as_empty() {
        let view = [rp::create(10, 0, 0x00FF_FFFF, 15, 0, 14), rp::EMPTY];
        let mut list = ReducingList::new();
        assert!(list.populate(&view));
        list.reduce(2);
        let mut out = [1u64; 3];
        list.copy_to(&mut out);
        assert_eq!(out, [rp::EMPTY; 3]);
    }

    #[test]
    fn overlap_is_rejected() {
        let view = [slab(60, 40, 1), slab(50, 0, 2)];
        let mut list = ReducingList::new();
        assert!(!list.populate(&view));
        let mut out = [1u64; 2];
        list.copy_to(&mut out);
        assert_eq!(out, [rp::EMPTY; 2]);
    }
}
