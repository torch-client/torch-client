use bevy::asset::{Asset, RenderAssetUsages, embedded_asset};
use bevy::camera::visibility::{NoFrustumCulling, RenderLayers};
use bevy::camera::{ClearColorConfig, RenderTarget};
use bevy::mesh::{Indices, Mesh, MeshVertexBufferLayoutRef, PrimitiveTopology};
use bevy::pbr::{Material, MaterialPipeline, MaterialPlugin};
use bevy::prelude::*;
use bevy::render::extract_resource::{ExtractResource, ExtractResourcePlugin};
use bevy::render::render_resource::{
    AsBindGroup, RenderPipelineDescriptor, ShaderType, SpecializedMeshPipelineError, TextureFormat,
};
use bevy::shader::ShaderRef;

use crate::gui::benchmark::{BUDGET_MS, GpuResult};
use crate::renderer::systems::{CameraStart, scene_camera};

const TILE: i32 = 256;
const MAX_COPIES: u32 = 1024;
const FIRST_SETTLE_FRAMES: u32 = 60;
const SETTLE_FRAMES: u32 = 20;
const FIRST_SETTLE_MS: f32 = 1000.0;
const SAMPLES: usize = 30;
const STEP_CAP_MS: f32 = 4000.0;
const BISECT_STEPS: u32 = 4;
const BASELINE_SHARE: f32 = 0.8;

const BENCH_LAYER: usize = 8;
const BENCH_CAMERA_ORDER: isize = 10;
const TARGET: UVec2 = UVec2::new(1920, 1080);
const SKY: LinearRgba = LinearRgba::new(0.36, 0.52, 0.86, 1.0);
const FOG_START: f32 = 600.0;
const FOG_END: f32 = 3000.0;

#[derive(Component)]
pub(crate) struct BenchmarkCamera;

#[derive(Resource, Clone, Default, ExtractResource)]
pub(crate) struct BenchImage(pub Option<Handle<Image>>);

const TURN_RATE: f32 = 0.1;
const PITCH: f32 = -0.5;

#[derive(Clone, Copy, Debug, Default, PartialEq, ShaderType)]
pub(super) struct BenchParams {
    sky: Vec4,
    fog: Vec4,
}

#[derive(Asset, TypePath, AsBindGroup, Clone)]
pub(super) struct BenchMaterial {
    #[uniform(0)]
    params: BenchParams,
}

impl Material for BenchMaterial {
    crate::renderer::material_shader_fns!();
    crate::renderer::material_no_prepass_no_shadows!();

    fn specialize(
        _pipeline: &MaterialPipeline,
        descriptor: &mut RenderPipelineDescriptor,
        layout: &MeshVertexBufferLayoutRef,
        _key: bevy::pbr::MaterialPipelineKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        let vertex_layout = layout.0.get_layout(&[
            Mesh::ATTRIBUTE_POSITION.at_shader_location(0),
            Mesh::ATTRIBUTE_NORMAL.at_shader_location(1),
            Mesh::ATTRIBUTE_COLOR.at_shader_location(2),
        ])?;
        descriptor.vertex.buffers = vec![vertex_layout];
        Ok(())
    }
}

fn shader() -> ShaderRef {
    crate::renderer::embedded_shader("renderer/benchmark/benchmark.wgsl")
}

pub(super) fn build(app: &mut App) {
    embedded_asset!(app, "benchmark.wgsl");
    app.add_plugins(MaterialPlugin::<BenchMaterial>::default())
        .init_resource::<BenchImage>()
        .add_plugins(ExtractResourcePlugin::<BenchImage>::default())
        .add_systems(Update, turn_camera);
}

fn turn_camera(
    time: Res<Time<bevy::time::Real>>,
    mut cameras: Query<&mut Transform, With<BenchmarkCamera>>,
    mut yaw: Local<f32>,
) {
    for mut transform in &mut cameras {
        *yaw = (*yaw + TURN_RATE * time.delta_secs()) % std::f32::consts::TAU;
        transform.rotation = Quat::from_rotation_y(*yaw) * Quat::from_rotation_x(PITCH);
    }
}

pub(super) struct GpuRun {
    camera: Entity,
    tiles: Vec<Entity>,
    spiral: Vec<IVec2>,
    mesh: Handle<Mesh>,
    material: Handle<BenchMaterial>,
    image: Handle<Image>,
    tile_triangles: u64,
    ramp: Ramp,
}

impl GpuRun {
    pub fn start(
        commands: &mut Commands,
        meshes: &mut Assets<Mesh>,
        materials: &mut Assets<BenchMaterial>,
        images: &mut Assets<Image>,
    ) -> Self {
        let (tile, tile_triangles) = tile_mesh();
        let mesh = meshes.add(tile);
        let material = materials.add(BenchMaterial {
            params: BenchParams {
                sky: Vec4::new(SKY.red, SKY.green, SKY.blue, 1.0),
                fog: Vec4::new(FOG_START, FOG_END, 0.0, 0.0),
            },
        });
        let image = images.add(Image::new_target_texture(
            TARGET.x,
            TARGET.y,
            TextureFormat::Rgba8UnormSrgb,
            None,
        ));
        let centre = TILE as f32 / 2.0;
        let camera = commands
            .spawn((
                BenchmarkCamera,
                scene_camera(
                    BENCH_CAMERA_ORDER,
                    ClearColorConfig::Custom(Color::LinearRgba(SKY)),
                    CameraStart::Active,
                    BENCH_LAYER,
                ),
                RenderTarget::Image(image.clone().into()),
                Projection::Perspective(PerspectiveProjection {
                    fov: 70.0_f32.to_radians(),
                    near: 0.1,
                    far: 16.0 * TILE as f32 * 2.0,
                    ..default()
                }),
                Transform::from_xyz(centre, 120.0, centre)
                    .with_rotation(Quat::from_rotation_x(PITCH)),
            ))
            .id();
        commands.insert_resource(BenchImage(Some(image.clone())));
        let mut run = Self {
            camera,
            tiles: Vec::with_capacity(MAX_COPIES as usize),
            spiral: spiral(MAX_COPIES as usize),
            mesh,
            material,
            image,
            tile_triangles,
            ramp: Ramp::new(),
        };
        run.set_count(commands, 1);
        run
    }

    pub fn frame(&mut self, commands: &mut Commands, frame_ms: f32) -> Option<GpuResult> {
        let median = self.ramp.step.feed(frame_ms)?;
        match self.ramp.finish_step(median) {
            Next::Count(count) => {
                self.set_count(commands, count);
                None
            }
            Next::Unmeasurable(baseline_ms) => Some(GpuResult::Unmeasurable { baseline_ms }),
            Next::Load { load, at_cap } => Some(GpuResult::Measured {
                load,
                triangles_per_frame: (f64::from(load) * self.tile_triangles as f64) as u64,
                at_cap,
                baseline_ms: self.ramp.baseline_ms,
                steps: self.ramp.steps_done,
            }),
        }
    }

    pub fn progress(&self) -> f32 {
        self.ramp.progress()
    }

    pub fn teardown(self, commands: &mut Commands) {
        commands.insert_resource(BenchImage(None));
        commands.entity(self.camera).try_despawn();
        for tile in self.tiles {
            commands.entity(tile).try_despawn();
        }
        drop((self.mesh, self.material, self.image));
    }

    fn set_count(&mut self, commands: &mut Commands, count: u32) {
        let count = (count as usize).min(self.spiral.len());
        while self.tiles.len() < count {
            let at = self.spiral[self.tiles.len()] * TILE;
            let tile = commands
                .spawn((
                    Mesh3d(self.mesh.clone()),
                    MeshMaterial3d(self.material.clone()),
                    Transform::from_xyz(at.x as f32, 0.0, at.y as f32),
                    NoFrustumCulling,
                    RenderLayers::layer(BENCH_LAYER),
                ))
                .id();
            self.tiles.push(tile);
        }
        while self.tiles.len() > count {
            if let Some(tile) = self.tiles.pop() {
                commands.entity(tile).try_despawn();
            }
        }
    }
}

fn spiral(n: usize) -> Vec<IVec2> {
    let mut out = Vec::with_capacity(n);
    let mut at = IVec2::ZERO;
    let mut dir = IVec2::X;
    let mut leg = 1;
    let mut walked = 0;
    let mut turns = 0;
    while out.len() < n {
        out.push(at);
        at += dir;
        walked += 1;
        if walked == leg {
            walked = 0;
            dir = IVec2::new(-dir.y, dir.x);
            turns += 1;
            if turns % 2 == 0 {
                leg += 1;
            }
        }
    }
    out
}

fn column(x: i32, z: i32) -> i32 {
    if !(0..TILE).contains(&x) || !(0..TILE).contains(&z) {
        return 0;
    }
    2 + (super::hash(x >> 4, 0, z >> 4, 11) % 12) as i32
        + (super::hash(x >> 1, 0, z >> 1, 12) % 3) as i32
}

fn tile_mesh() -> (Mesh, u64) {
    let mut b = Builder::default();
    for x in 0..TILE {
        for z in 0..TILE {
            let h = column(x, z);
            let t = (h as f32 / 16.0).clamp(0.0, 1.0);
            let top = [0.30 + 0.32 * t, 0.52 + 0.06 * t, 0.20 + 0.36 * t, 1.0];
            let side = [top[0] * 0.8, top[1] * 0.8, top[2] * 0.8, 1.0];
            let (x0, x1, z0, z1, y) =
                (x as f32, (x + 1) as f32, z as f32, (z + 1) as f32, h as f32);
            b.quad(
                [[x0, y, z0], [x0, y, z1], [x1, y, z1], [x1, y, z0]],
                Vec3::Y,
                top,
            );
            for (dx, dz) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                let low = column(x + dx, z + dz);
                if low >= h {
                    continue;
                }
                let y0 = low as f32;
                let normal = Vec3::new(dx as f32, 0.0, dz as f32);
                let corners = if dx != 0 {
                    let a = if dx > 0 { x1 } else { x0 };
                    [[a, y0, z0], [a, y, z0], [a, y, z1], [a, y0, z1]]
                } else {
                    let a = if dz > 0 { z1 } else { z0 };
                    [[x0, y0, a], [x0, y, a], [x1, y, a], [x1, y0, a]]
                };
                b.quad(corners, normal, side);
            }
        }
    }
    let triangles = (b.indices.len() / 3) as u64;
    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::RENDER_WORLD,
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, b.positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, b.normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, b.colors);
    mesh.insert_indices(Indices::U32(b.indices));
    (mesh, triangles)
}

#[derive(Default)]
struct Builder {
    positions: Vec<[f32; 3]>,
    normals: Vec<[f32; 3]>,
    colors: Vec<[f32; 4]>,
    indices: Vec<u32>,
}

impl Builder {
    fn quad(&mut self, corners: [[f32; 3]; 4], normal: Vec3, color: [f32; 4]) {
        let base = self.positions.len() as u32;
        let [p0, p1, p2, _] = corners.map(Vec3::from);
        let ccw = (p1 - p0).cross(p2 - p0).dot(normal) > 0.0;
        let order: [u32; 6] = if ccw {
            [0, 1, 2, 0, 2, 3]
        } else {
            [0, 2, 1, 0, 3, 2]
        };
        self.positions.extend(corners);
        self.normals.extend([normal.to_array(); 4]);
        self.colors.extend([color; 4]);
        self.indices.extend(order.map(|i| base + i));
    }
}

struct Step {
    count: u32,
    settle: u32,
    settle_total: u32,
    settle_ms: f32,
    elapsed_ms: f32,
    frames: u32,
    samples: Vec<f32>,
}

impl Step {
    fn new(count: u32, first: bool) -> Self {
        let settle = if first {
            FIRST_SETTLE_FRAMES
        } else {
            SETTLE_FRAMES
        };
        Self {
            count,
            settle,
            settle_total: settle,
            settle_ms: if first { FIRST_SETTLE_MS } else { 0.0 },
            elapsed_ms: 0.0,
            frames: 0,
            samples: Vec::with_capacity(SAMPLES),
        }
    }

    fn feed(&mut self, ms: f32) -> Option<f32> {
        self.elapsed_ms += ms;
        self.frames += 1;
        if self.settle > 0 || self.elapsed_ms < self.settle_ms {
            self.settle = self.settle.saturating_sub(1);
        } else {
            self.samples.push(ms);
        }
        if self.samples.len() < SAMPLES && self.elapsed_ms < STEP_CAP_MS {
            return None;
        }
        if self.samples.is_empty() {
            self.samples.push(ms);
        }
        Some(median(&mut self.samples))
    }

    fn fraction(&self) -> f32 {
        let by_frames = self.frames as f32 / (self.settle_total as usize + SAMPLES) as f32;
        let by_time = self.elapsed_ms / STEP_CAP_MS;
        by_frames.max(by_time).min(0.99)
    }
}

fn median(samples: &mut [f32]) -> f32 {
    samples.sort_unstable_by(f32::total_cmp);
    let mid = samples.len() / 2;
    if samples.len() % 2 == 0 {
        (samples[mid - 1] + samples[mid]) / 2.0
    } else {
        samples[mid]
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Search {
    Baseline,
    Doubling,
    Bisect,
}

enum Next {
    Count(u32),
    Unmeasurable(f32),
    Load { load: f32, at_cap: bool },
}

struct Ramp {
    search: Search,
    step: Step,
    under: (u32, f32),
    over: (u32, f32),
    bisect_left: u32,
    steps_done: u32,
    baseline_ms: f32,
}

impl Ramp {
    fn new() -> Self {
        Self {
            search: Search::Baseline,
            step: Step::new(1, true),
            under: (1, 0.0),
            over: (MAX_COPIES, 0.0),
            bisect_left: BISECT_STEPS,
            steps_done: 0,
            baseline_ms: 0.0,
        }
    }

    fn finish_step(&mut self, median: f32) -> Next {
        let count = self.step.count;
        self.steps_done += 1;
        match self.search {
            Search::Baseline => {
                if median > BASELINE_SHARE * BUDGET_MS {
                    return Next::Unmeasurable(median);
                }
                self.baseline_ms = median;
                self.under = (count, median);
                self.search = Search::Doubling;
                self.begin(count * 2)
            }
            Search::Doubling => {
                if median > BUDGET_MS {
                    self.over = (count, median);
                    self.search = Search::Bisect;
                    return self.bisect();
                }
                self.under = (count, median);
                if count >= MAX_COPIES {
                    return Next::Load {
                        load: MAX_COPIES as f32,
                        at_cap: true,
                    };
                }
                self.begin((count * 2).min(MAX_COPIES))
            }
            Search::Bisect => {
                if median > BUDGET_MS {
                    self.over = (count, median);
                } else {
                    self.under = (count, median);
                }
                self.bisect_left -= 1;
                self.bisect()
            }
        }
    }

    fn bisect(&mut self) -> Next {
        let (under, under_ms) = self.under;
        let (over, over_ms) = self.over;
        let mid = (under + over) / 2;
        if self.bisect_left == 0 || mid == under {
            let t = if over_ms > under_ms {
                ((BUDGET_MS - under_ms) / (over_ms - under_ms)).clamp(0.0, 1.0)
            } else {
                0.0
            };
            let load = under as f32 + (over - under) as f32 * t;
            return Next::Load {
                load,
                at_cap: false,
            };
        }
        self.begin(mid)
    }

    fn begin(&mut self, count: u32) -> Next {
        self.step = Step::new(count, false);
        Next::Count(count)
    }

    fn progress(&self) -> f32 {
        let remaining = match self.search {
            Search::Baseline => 1 + MAX_COPIES.ilog2() + BISECT_STEPS,
            Search::Doubling => MAX_COPIES.ilog2() - self.step.count.ilog2() + 1 + BISECT_STEPS,
            Search::Bisect => self.bisect_left,
        };
        let done = self.steps_done as f32 + self.step.fraction();
        (done / (self.steps_done + remaining).max(1) as f32).clamp(0.0, 1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(cost: impl Fn(u32) -> f32) -> (Next, Vec<u32>) {
        let mut ramp = Ramp::new();
        let mut counts = vec![1];
        loop {
            let ms = cost(ramp.step.count);
            if let Some(median) = ramp.step.feed(ms) {
                match ramp.finish_step(median) {
                    Next::Count(c) => counts.push(c),
                    other => return (other, counts),
                }
            }
        }
    }

    #[test]
    fn the_spiral_starts_at_the_origin_and_never_repeats() {
        let s = spiral(MAX_COPIES as usize);
        assert_eq!(
            &s[..5],
            &[
                IVec2::ZERO,
                IVec2::X,
                IVec2::ONE,
                IVec2::Y,
                IVec2::new(-1, 1)
            ]
        );
        let unique: std::collections::HashSet<_> = s.iter().copied().collect();
        assert_eq!(unique.len(), s.len());
    }

    #[test]
    fn a_linear_cost_lands_on_the_budget() {
        let (next, counts) = run(|c| 5.0 + 0.1 * c as f32);
        assert_eq!(&counts[..10], &[1, 2, 4, 8, 16, 32, 64, 128, 256, 512]);
        let Next::Load { load, at_cap } = next else {
            panic!("no load")
        };
        assert!(!at_cap);
        assert!((load - 300.0).abs() < 1.0, "{load}");
    }

    #[test]
    fn a_cheap_scene_stops_at_the_cap() {
        let (next, _) = run(|_| 2.0);
        assert!(matches!(next, Next::Load { at_cap: true, .. }));
    }

    #[test]
    fn a_slow_baseline_is_unmeasurable() {
        let (next, counts) = run(|_| 30.0);
        assert_eq!(counts, vec![1]);
        assert!(matches!(next, Next::Unmeasurable(ms) if ms == 30.0));
    }
}
