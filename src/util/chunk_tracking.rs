pub(crate) fn is_within_distance(
    center_x: i32,
    center_z: i32,
    view_distance: u32,
    chunk_x: i32,
    chunk_z: i32,
    include_neighbors: bool,
) -> bool {
    let buffer: i64 = if include_neighbors { 2 } else { 1 };
    let dx = ((chunk_x as i64 - center_x as i64).abs() - buffer).max(0);
    let dz = ((chunk_z as i64 - center_z as i64).abs() - buffer).max(0);
    dx * dx + dz * dz < (view_distance as i64) * (view_distance as i64)
}

pub(crate) fn is_in_view_distance(
    center_x: i32,
    center_z: i32,
    view_distance: u32,
    chunk_x: i32,
    chunk_z: i32,
) -> bool {
    is_within_distance(center_x, center_z, view_distance, chunk_x, chunk_z, false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_buffer_is_free() {
        assert!(is_in_view_distance(0, 0, 1, 1, 0));
        assert!(is_in_view_distance(0, 0, 1, 1, 1));
        assert!(!is_in_view_distance(0, 0, 1, 2, 0));
        assert!(is_in_view_distance(0, 0, 2, 2, 2));
        assert!(!is_in_view_distance(0, 0, 2, 3, 3));
        assert!(is_within_distance(0, 0, 2, 3, 3, true));
    }
}
