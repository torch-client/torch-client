use bevy::render::renderer::RenderDevice;

use super::compile::{EntityKind, ProgramKind, Step};
use super::install::{Installed, InstalledProgram};
use super::sources::{Depth, TextureSource};
use crate::shaderpack::directives::{MAX_COLOR_TARGETS, SampleKind};
use crate::shaderpack::pipeline::Binding;
use crate::shaderpack::transform::ImageAccess;

pub(super) struct Usage {
    pub(super) shadow: bool,
    pub(super) shadow_filtered: [bool; 2],
    pub(super) image_conflicts: Vec<[u16; 2]>,
    pub(super) snapshots: u16,
    pub(super) mip_targets: u16,
    pub(super) cleared: u16,
    pub(super) copy_depth: bool,
    pub(super) copy_no_hand: bool,
    pub(super) shadow_voxelizes: bool,
    pub(super) shadow_copy_opaque: bool,
    pub(super) kind_blocks: [Option<usize>; EntityKind::COUNT],
}

impl Usage {
    pub(super) fn of(device: &RenderDevice, installed: &Installed) -> Usage {
        let vertex_storage = device
            .features()
            .contains(wgpu::Features::VERTEX_WRITABLE_STORAGE);
        let shadow = installed.shadow.is_some_and(|index| {
            let usable = vertex_storage || !writes_images_from_vertex(&installed.programs[index]);
            if !usable {
                crate::log_warn!(
                    "shaders",
                    "no shadows: the shadow program writes images from its vertex stage, which this GPU cannot"
                );
            }
            usable
        });
        let shadow_filtered = if shadow {
            shadow_filtering(device, installed)
        } else {
            [false; 2]
        };
        let image_conflicts: Vec<[u16; 2]> = installed
            .programs
            .iter()
            .map(|p| match &p.compute_parity {
                Some(variants) => variants
                    .each_ref()
                    .map(|(_, used)| image_conflicts(installed, p, Some(used))),
                None => [image_conflicts(installed, p, p.compute_used.as_deref()); 2],
            })
            .collect();
        let reads = |wanted: &dyn Fn(ProgramKind, TextureSource) -> bool| {
            installed
                .programs
                .iter()
                .any(|p| p.textures.iter().any(|(_, source)| wanted(p.kind, *source)))
        };
        Usage {
            shadow,
            shadow_filtered,
            snapshots: image_conflicts.iter().flatten().fold(0, |mask, m| mask | m),
            image_conflicts,
            mip_targets: mip_targets(installed),
            cleared: (0..MAX_COLOR_TARGETS as u8)
                .filter(|n| {
                    installed.used_targets & (1u16 << *n) != 0
                        && installed.targets.settings[*n as usize].clear
                })
                .fold(0, |mask, n| mask | 1 << n),
            copy_depth: reads(&|kind, source| match source {
                TextureSource::Depth(Depth::Opaque) => true,
                TextureSource::Depth(Depth::All) => kind != ProgramKind::Screen,
                _ => false,
            }),
            copy_no_hand: reads(&|_, source| source == TextureSource::DepthNoHand),
            shadow_voxelizes: installed.shadow.is_some_and(|index| {
                installed.programs[index]
                    .layout
                    .bindings
                    .iter()
                    .any(|b| matches!(b, Binding::Image { .. }))
            }),
            shadow_copy_opaque: !shadow_filtered[1]
                && reads(&|kind, source| {
                    !kind.is_shadow() && source == TextureSource::ShadowDepth(Depth::Opaque)
                }),
            kind_blocks: {
                let mut extra = installed.programs.len();
                std::array::from_fn(|at| match installed.entities[at] {
                    Some(_) if EntityKind::ALL[at].is_hand() => {
                        extra += 1;
                        Some(extra - 1)
                    }
                    other => other,
                })
            },
        }
    }
}

fn shadow_filtering(device: &RenderDevice, installed: &Installed) -> [bool; 2] {
    if installed.shadow.is_none() || !super::depthcopy::supported(device) {
        return [false; 2];
    }
    [Depth::All, Depth::Opaque].map(|depth| {
        !installed.constants.shadow_nearest[depth_index(depth)]
            && installed.programs.iter().any(|p| {
                !p.kind.is_shadow()
                    && p.textures
                        .iter()
                        .any(|(_, s)| *s == TextureSource::ShadowDepth(depth))
            })
    })
}

pub(super) fn depth_index(depth: Depth) -> usize {
    match depth {
        Depth::All => 0,
        Depth::Opaque => 1,
    }
}

fn image_conflicts(installed: &Installed, program: &InstalledProgram, used: Option<&[u32]>) -> u16 {
    let reached = |binding: u32| used.is_none_or(|used| used.contains(&binding));
    let written = |index: usize| {
        let name = &installed.custom_images[index].name;
        program.layout.bindings.iter().any(|b| {
            matches!(b, Binding::Image { name: n, access, .. } if n == name && *access != ImageAccess::Read)
                && reached(b.binding())
        })
    };
    program
        .textures
        .iter()
        .filter(|(binding, _)| reached(*binding))
        .filter_map(|(_, source)| match source {
            TextureSource::Image { index, .. } if written(usize::from(*index)) => {
                Some(1u16 << index)
            }
            _ => None,
        })
        .fold(0, |mask, bit| mask | bit)
}

fn writes_images_from_vertex(program: &InstalledProgram) -> bool {
    program.layout.bindings.iter().any(|b| {
        matches!(b, Binding::Image { vertex: true, access, .. } if *access != ImageAccess::Read)
            || matches!(
                b,
                Binding::Buffer {
                    vertex: true,
                    read_only: false,
                    ..
                }
            )
    })
}

fn mip_targets(installed: &Installed) -> u16 {
    let screen = installed
        .stages
        .iter()
        .flatten()
        .filter_map(|step| match step {
            Step::Screen(index) => Some(index),
            Step::Compute(_) => None,
        })
        .chain(&installed.final_pass)
        .fold(0u16, |mask, index| {
            mask | installed.programs[*index].mipmapped
        });
    (0..MAX_COLOR_TARGETS)
        .filter(|n| {
            screen & (1 << n) != 0
                && installed.used_targets & (1 << n) != 0
                && installed.targets.settings[*n].format.sample_kind() == SampleKind::Float
        })
        .fold(0, |mask, n| mask | 1 << n)
}
