use std::sync::Arc;

pub const SIZE: usize = 2048;
const CELLS: usize = 4096;

#[inline]
const fn index(x: usize, y: usize, z: usize) -> usize {
    debug_assert!(y << 8 | z << 4 | x < CELLS);
    y << 8 | z << 4 | x
}

#[derive(Clone, PartialEq, Eq)]
pub struct DataLayer {
    data: Option<Arc<[u8; SIZE]>>,
    default: u8,
}

impl DataLayer {
    pub const fn empty() -> Self {
        Self {
            data: None,
            default: 0,
        }
    }

    pub const fn filled(default: u8) -> Self {
        Self {
            data: None,
            default,
        }
    }

    pub fn from_bytes(data: &[u8; SIZE]) -> Self {
        Self {
            data: Some(Arc::new(*data)),
            default: 0,
        }
    }

    pub fn from_wire(data: &[u8; SIZE]) -> Self {
        match uniform_nibble(data) {
            Some(value) => Self::filled(value),
            None => Self::from_bytes(data),
        }
    }

    pub fn bytes(&self) -> u64 {
        if self.data.is_some() { SIZE as u64 } else { 0 }
    }

    pub fn unshared_bytes(&self) -> u64 {
        match &self.data {
            Some(d) if Arc::strong_count(d) == 1 => SIZE as u64,
            _ => 0,
        }
    }

    #[inline]
    pub fn get(&self, x: usize, y: usize, z: usize) -> u8 {
        let i = index(x, y, z);
        match &self.data {
            None => self.default,
            Some(d) => (d[i >> 1] >> ((i & 1) * 4)) & 15,
        }
    }

    #[inline]
    pub fn set(&mut self, x: usize, y: usize, z: usize, value: u8) {
        let i = index(x, y, z);
        let d = self.materialise();
        let shift = (i & 1) * 4;
        d[i >> 1] = (d[i >> 1] & !(15 << shift)) | ((value & 15) << shift);
    }

    pub fn fill(&mut self, value: u8) {
        self.data = None;
        self.default = value;
    }

    pub fn is_empty(&self) -> bool {
        self.data.is_none() && self.default == 0
    }

    pub fn is_definitely_homogenous(&self) -> bool {
        self.data.is_none()
    }

    #[cfg(target_arch = "wasm32")]
    pub fn as_bytes(&self) -> Option<&[u8; SIZE]> {
        self.data.as_deref()
    }

    #[cfg(target_arch = "wasm32")]
    pub fn fill_value(&self) -> u8 {
        self.default
    }

    fn materialise(&mut self) -> &mut [u8; SIZE] {
        let byte = self.default | (self.default << 4);
        Arc::make_mut(self.data.get_or_insert_with(|| Arc::new([byte; SIZE])))
    }

    pub fn repeat_first_layer(&self) -> Self {
        if self.is_definitely_homogenous() {
            return self.clone();
        }
        let src = self.data.as_ref().expect("not homogenous");
        let mut out = Arc::new([0u8; SIZE]);
        let dst = Arc::make_mut(&mut out);
        for slice in 0..16 {
            dst[slice * 128..(slice + 1) * 128].copy_from_slice(&src[..128]);
        }
        Self {
            data: Some(out),
            default: 0,
        }
    }
}

fn uniform_nibble(data: &[u8; SIZE]) -> Option<u8> {
    let first = data[0];
    ((first >> 4) == (first & 15) && data.iter().all(|&b| b == first)).then_some(first & 15)
}

impl Default for DataLayer {
    fn default() -> Self {
        Self::empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_filled_layer_reads_back_its_default_everywhere() {
        let layer = DataLayer::filled(15);
        assert_eq!(layer.get(0, 0, 0), 15);
        assert_eq!(layer.get(15, 15, 15), 15);
        assert!(!layer.is_empty());
    }

    #[test]
    fn neighbouring_nibbles_do_not_collide() {
        let mut layer = DataLayer::empty();
        layer.set(0, 0, 0, 7);
        layer.set(1, 0, 0, 12);
        assert_eq!(layer.get(0, 0, 0), 7);
        assert_eq!(layer.get(1, 0, 0), 12);
    }

    #[test]
    fn materialising_keeps_the_fill() {
        let mut layer = DataLayer::filled(15);
        layer.set(3, 4, 5, 2);
        assert_eq!(layer.get(3, 4, 5), 2);
        assert_eq!(layer.get(4, 4, 5), 15);
    }

    #[test]
    fn a_uniform_wire_layer_collapses_to_its_fill() {
        let lit = DataLayer::from_wire(&[0xFF; SIZE]);
        assert!(lit.is_definitely_homogenous());
        assert_eq!(lit.bytes(), 0);
        assert_eq!(lit.get(7, 7, 7), 15);

        let dark = DataLayer::from_wire(&[0; SIZE]);
        assert!(dark.is_definitely_homogenous());
        assert_eq!(dark.get(0, 15, 0), 0);

        let striped = DataLayer::from_wire(&[0x10; SIZE]);
        assert!(!striped.is_definitely_homogenous());
        assert_eq!(striped.get(0, 0, 0), 0);
        assert_eq!(striped.get(1, 0, 0), 1);

        let mut one_off = [0x77; SIZE];
        one_off[SIZE - 1] = 0x78;
        let one_off = DataLayer::from_wire(&one_off);
        assert!(!one_off.is_definitely_homogenous());
        assert_eq!(one_off.get(15, 15, 15), 7);
        assert_eq!(one_off.get(14, 15, 15), 8);
    }

    #[test]
    fn a_write_after_clone_copies_and_leaves_the_clone_alone() {
        let mut engine = DataLayer::empty();
        engine.set(1, 2, 3, 9);
        let published = engine.clone();
        assert_eq!(engine.unshared_bytes(), 0, "shared after the clone");
        engine.set(1, 2, 3, 4);
        assert_eq!(engine.get(1, 2, 3), 4);
        assert_eq!(published.get(1, 2, 3), 9);
        assert_eq!(engine.unshared_bytes(), SIZE as u64);
        assert_eq!(published.unshared_bytes(), SIZE as u64);
    }

    #[test]
    fn repeat_first_layer_copies_the_bottom_slice_upward() {
        let mut layer = DataLayer::empty();
        layer.set(2, 0, 3, 9);
        let repeated = layer.repeat_first_layer();
        for y in 0..16 {
            assert_eq!(repeated.get(2, y, 3), 9, "y={y}");
        }
    }
}
