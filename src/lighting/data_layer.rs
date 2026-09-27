pub const SIZE: usize = 2048;
const CELLS: usize = 4096;

#[inline]
const fn index(x: usize, y: usize, z: usize) -> usize {
    debug_assert!(y << 8 | z << 4 | x < CELLS);
    y << 8 | z << 4 | x
}

#[derive(Clone, PartialEq, Eq)]
pub struct DataLayer {
    data: Option<Box<[u8; SIZE]>>,
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

    pub fn from_bytes(data: Box<[u8; SIZE]>) -> Self {
        Self {
            data: Some(data),
            default: 0,
        }
    }

    pub fn bytes(&self) -> u64 {
        if self.data.is_some() { SIZE as u64 } else { 0 }
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
        self.data.get_or_insert_with(|| Box::new([byte; SIZE]))
    }

    pub fn repeat_first_layer(&self) -> Self {
        if self.is_definitely_homogenous() {
            return self.clone();
        }
        let src = self.data.as_ref().expect("not homogenous");
        let mut out = Box::new([0u8; SIZE]);
        for slice in 0..16 {
            out[slice * 128..(slice + 1) * 128].copy_from_slice(&src[..128]);
        }
        Self::from_bytes(out)
    }
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
    fn repeat_first_layer_copies_the_bottom_slice_upward() {
        let mut layer = DataLayer::empty();
        layer.set(2, 0, 3, 9);
        let repeated = layer.repeat_first_layer();
        for y in 0..16 {
            assert_eq!(repeated.get(2, y, 3), 9, "y={y}");
        }
    }
}
