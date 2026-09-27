pub mod animations;
pub mod armadillo;
pub mod camel;
pub mod chicken;
pub mod cow;
pub mod equine;
pub mod feline;
pub mod fox;
pub mod goat;
pub mod hoglin;
pub mod llama;
pub mod panda;
pub mod polar_bear;
pub mod quadruped;
pub mod rabbit;
pub mod sheep;
pub mod sniffer;
pub mod strider;
pub mod turtle;
pub mod wolf;

use crate::entities::geom::MeshDef;

pub fn scaling(mesh: MeshDef, factor: f32) -> MeshDef {
    let y_offset = 24.016 * (1.0 - factor);
    mesh.transformed(|pose| pose.scaled(factor).translated(0.0, y_offset, 0.0))
}
