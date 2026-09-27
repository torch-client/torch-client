use crate::entities::geom::MeshDef;
use crate::util::mth::DEG_TO_RAD;

pub mod allay;
pub mod animation;
pub mod axolotl;
pub mod bat;
pub mod bee;
pub mod dolphin;
pub mod fish;
pub mod frog;
pub mod ghast;
pub mod nautilus;
pub mod parrot;
pub mod squid;

pub fn scaling(mesh: MeshDef, factor: f32) -> MeshDef {
    let y_offset = 24.016 * (1.0 - factor);
    mesh.transformed(|pose| pose.scaled(factor).translated(0.0, y_offset, 0.0))
}
