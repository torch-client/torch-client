use super::point;
use super::pos::{SectionPos, WIDTH, column_index};

const COLUMNS: usize = (WIDTH * WIDTH) as usize;

pub struct FullSection {
    pub pos: SectionPos,
    starts: Vec<u32>,
    points: Vec<u64>,
    pub is_empty: bool,
}

impl FullSection {
    pub fn new(pos: SectionPos) -> Self {
        Self {
            pos,
            starts: vec![0; COLUMNS + 1],
            points: Vec::new(),
            is_empty: true,
        }
    }

    #[inline]
    pub fn column(&self, x: i32, z: i32) -> &[u64] {
        let i = column_index(x, z);
        &self.points[self.starts[i] as usize..self.starts[i + 1] as usize]
    }

    pub fn replace_block(&mut self, x0: i32, z0: i32, w: i32, columns: &[Vec<u64>]) -> bool {
        debug_assert_eq!(columns.len(), (w * w) as usize);
        let changed = (0..w).any(|dx| {
            (0..w).any(|dz| {
                self.column(x0 + dx, z0 + dz) != columns[(dx * w + dz) as usize].as_slice()
            })
        });
        if !changed {
            return false;
        }
        let mut starts = Vec::with_capacity(COLUMNS + 1);
        let mut points = Vec::with_capacity(self.points.len());
        for x in 0..WIDTH {
            for z in 0..WIDTH {
                starts.push(points.len() as u32);
                let inside = (x0..x0 + w).contains(&x) && (z0..z0 + w).contains(&z);
                if inside {
                    points.extend_from_slice(&columns[((x - x0) * w + (z - z0)) as usize]);
                } else {
                    points.extend_from_slice(self.column(x, z));
                }
            }
        }
        starts.push(points.len() as u32);
        self.starts = starts;
        self.points = points;
        true
    }

    pub fn update_from_child(
        &mut self,
        child: &FullSection,
        is_air: impl Fn(u32) -> bool + Copy,
    ) -> bool {
        debug_assert_eq!(child.pos.parent(), self.pos);
        let offset_x = if child.pos.x == self.pos.x * 2 {
            0
        } else {
            WIDTH / 2
        };
        let offset_z = if child.pos.z == self.pos.z * 2 {
            0
        } else {
            WIDTH / 2
        };
        let half = WIDTH / 2;
        let mut merged = Vec::with_capacity((half * half) as usize);
        for rx in 0..half {
            for rz in 0..half {
                merged.push(merge_two_by_two(child, rx * 2, rz * 2, is_air));
            }
        }
        let changed = self.replace_block(offset_x, offset_z, half, &merged);
        if !child.is_empty {
            self.is_empty = false;
        }
        changed
    }
}

fn merge_two_by_two(input: &FullSection, x: i32, z: i32, is_air: impl Fn(u32) -> bool) -> Vec<u64> {
    let columns = [
        input.column(x, z),
        input.column(x, z + 1),
        input.column(x + 1, z),
        input.column(x + 1, z + 1),
    ];
    let mut out = Vec::new();

    let mut transitions: Vec<i32> = Vec::new();
    for column in columns {
        for &p in column {
            let min_y = point::bottom_y(p);
            let max_y = min_y + point::height(p);
            transitions.push(min_y);
            transitions.push(max_y);
        }
    }
    if transitions.is_empty() {
        return out;
    }
    transitions.sort_unstable();
    transitions.dedup();

    let mut indices: [i32; 4] = std::array::from_fn(|i| columns[i].len() as i32 - 1);

    let mut last_id = 0u32;
    let mut last_block_light = 0u8;
    let mut last_sky_light = 0u8;
    let mut current_min_y = transitions[0];
    let mut accumulated = 0i32;

    for pair in transitions.windows(2) {
        let (slice_min, slice_max) = (pair[0], pair[1]);
        let slice_height = slice_max - slice_min;
        let sample_y = slice_min + slice_height / 2;

        let mut ids = [0u32; 4];
        let mut block_lights = [0u8; 4];
        let mut sky_lights = [0u8; 4];
        for i in 0..4 {
            let column = columns[i];
            while indices[i] >= 0 {
                let p = column[indices[i] as usize];
                let min_y = point::bottom_y(p);
                let max_y = min_y + point::height(p);
                if sample_y >= max_y {
                    indices[i] -= 1;
                } else if sample_y >= min_y {
                    ids[i] = point::id(p);
                    block_lights[i] = point::block_light(p);
                    sky_lights[i] = point::sky_light(p);
                    break;
                } else {
                    break;
                }
            }
        }

        let id = most_common(&ids, &is_air);
        let block_light = average_with_id(&block_lights, &ids, id);
        let sky_light = average_with_id(&sky_lights, &ids, id);

        if accumulated == 0 {
            last_id = id;
            last_block_light = block_light;
            last_sky_light = sky_light;
            current_min_y = slice_min;
            accumulated = slice_height;
        } else if id != last_id || block_light != last_block_light || sky_light != last_sky_light {
            out.push(point::encode(
                last_id,
                accumulated,
                current_min_y,
                last_block_light,
                last_sky_light,
            ));
            last_id = id;
            last_block_light = block_light;
            last_sky_light = sky_light;
            current_min_y = slice_min;
            accumulated = slice_height;
        } else {
            accumulated += slice_height;
        }
    }
    if accumulated > 0 {
        out.push(point::encode(
            last_id,
            accumulated,
            current_min_y,
            last_block_light,
            last_sky_light,
        ));
    }
    out.reverse();
    out
}

fn most_common(ids: &[u32; 4], is_air: impl Fn(u32) -> bool) -> u32 {
    let mut counts = [0u32; 4];
    for &v in ids {
        if is_air(v) {
            continue;
        }
        if v == ids[0] {
            counts[0] += 1;
        } else if v == ids[1] {
            counts[1] += 1;
        } else if v == ids[2] {
            counts[2] += 1;
        } else {
            counts[3] += 1;
        }
    }
    let max = *counts.iter().max().expect("four counts");
    if max == counts[0] {
        ids[0]
    } else if max == counts[1] {
        ids[1]
    } else if max == counts[2] {
        ids[2]
    } else {
        ids[3]
    }
}

fn average_with_id(values: &[u8; 4], ids: &[u32; 4], winner: u32) -> u8 {
    let (mut sum, mut count) = (0u32, 0u32);
    for i in 0..4 {
        if ids[i] == winner {
            sum += values[i] as u32;
            count += 1;
        }
    }
    if count == 0 {
        return (values.iter().map(|&v| v as u32).sum::<u32>() / 4) as u8;
    }
    (sum / count) as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    fn air(id: u32) -> bool {
        id == 0
    }

    #[test]
    fn replace_block_keeps_untouched_columns() {
        let mut s = FullSection::new(SectionPos::new(0, 0, 0));
        let a = vec![point::encode(5, 10, 0, 0, 15)];
        assert!(s.replace_block(0, 0, 1, &[a.clone()]));
        let b = vec![point::encode(6, 3, 2, 0, 15)];
        assert!(s.replace_block(1, 1, 1, &[b.clone()]));
        assert_eq!(s.column(0, 0), a.as_slice());
        assert_eq!(s.column(1, 1), b.as_slice());
        assert!(s.column(2, 2).is_empty());
        assert!(!s.replace_block(1, 1, 1, &[b]));
    }

    #[test]
    fn merge_takes_majority_and_stays_top_down() {
        let mut child = FullSection::new(SectionPos::new(0, 0, 0));
        let stone = vec![
            point::encode(0, 10, 10, 0, 15),
            point::encode(1, 10, 0, 0, 0),
        ];
        let dirt = vec![
            point::encode(0, 10, 10, 0, 15),
            point::encode(2, 10, 0, 0, 0),
        ];
        child.replace_block(0, 0, 2, &[stone.clone(), stone.clone(), stone, dirt]);
        let merged = merge_two_by_two(&child, 0, 0, air);
        assert_eq!(merged.len(), 2);
        assert!(point::bottom_y(merged[0]) > point::bottom_y(merged[1]));
        assert_eq!(point::id(merged[1]), 1);
        assert_eq!(point::height(merged[1]), 10);
    }
}
