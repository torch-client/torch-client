mod cpu;
mod gpu;

use bevy::prelude::*;
use bevy::render::renderer::RenderAdapterInfo;
use bevy::time::Real;

use crate::gui::benchmark::{BenchResult, CpuStats, Phase};
use crate::gui::{GuiState, Screen};
use crate::platform::time::Instant;

pub(crate) use gpu::{BenchImage, BenchmarkCamera};

const CPU_SHARE: f32 = 0.3;

const SEED: i32 = 0x5EED;

fn hash(x: i32, y: i32, z: i32, salt: i32) -> u32 {
    let h =
        crate::blocks::rand::position_seed(x ^ SEED, y.wrapping_add(salt.wrapping_mul(7919)), z);
    (h ^ (h >> 23)) as u32
}

pub struct BenchmarkPlugin;

impl Plugin for BenchmarkPlugin {
    fn build(&self, app: &mut App) {
        gpu::build(app);
        app.add_systems(
            Update,
            drive.run_if(resource_exists::<super::systems::AssetsReady>),
        );
    }
}

enum Stage {
    Cpu(Box<cpu::CpuRun>),
    Gpu { run: gpu::GpuRun, cpu: CpuStats },
}

struct Live {
    stage: Stage,
    progress: f32,
    started: Instant,
}

impl Live {
    fn teardown(self, commands: &mut Commands) {
        if let Stage::Gpu { run, .. } = self.stage {
            run.teardown(commands);
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn drive(
    mut commands: Commands,
    mut gui: ResMut<GuiState>,
    time: Res<Time<Real>>,
    adapter: Option<Res<RenderAdapterInfo>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<gpu::BenchMaterial>>,
    mut images: ResMut<Assets<Image>>,
    mut live: Local<Option<Live>>,
) {
    let on_screen = gui.screen == Screen::Benchmark;
    let phase = gui.benchmark.phase;
    if live.is_some() && (!on_screen || phase == Phase::Idle) {
        if let Some(run) = live.take() {
            run.teardown(&mut commands);
        }
        if phase != Phase::Idle {
            gui.benchmark.phase = Phase::Idle;
        }
        return;
    }
    if live.is_none() {
        if !on_screen || phase != Phase::Cpu {
            return;
        }
        *live = Some(Live {
            started: Instant::now(),
            stage: Stage::Cpu(Box::new(cpu::CpuRun::new())),
            progress: 0.0,
        });
    }
    let Some(state) = live.as_mut() else { return };

    let mut next = None;
    let mut finished = None;
    let progress = match &mut state.stage {
        Stage::Cpu(run) => match run.step() {
            Some(cpu) => {
                let run =
                    gpu::GpuRun::start(&mut commands, &mut meshes, &mut materials, &mut images);
                next = Some(Stage::Gpu { run, cpu });
                CPU_SHARE
            }
            None => CPU_SHARE * run.progress(),
        },
        Stage::Gpu { run, cpu } => {
            let frame_ms = time.delta().as_secs_f32() * 1000.0;
            match run.frame(&mut commands, frame_ms) {
                Some(gpu) => {
                    finished = Some(BenchResult {
                        cpu: *cpu,
                        gpu,
                        adapter: adapter_name(adapter.as_deref()),
                        cores: cores(),
                        seconds: state.started.elapsed().as_secs_f32(),
                    });
                    1.0
                }
                None => CPU_SHARE + (1.0 - CPU_SHARE) * run.progress(),
            }
        }
    };
    if let Some(stage) = next {
        state.stage = stage;
        gui.benchmark.phase = Phase::Gpu;
    }
    state.progress = state.progress.max(progress);
    if gui.benchmark.progress != state.progress {
        gui.benchmark.progress = state.progress;
    }
    if let Some(result) = finished {
        if let Some(run) = live.take() {
            run.teardown(&mut commands);
        }
        gui.benchmark.result = Some(result);
        gui.benchmark.phase = Phase::Done;
    }
}

fn cores() -> Option<u32> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        u32::try_from(crate::diag::core_count()).ok()
    }
    #[cfg(target_arch = "wasm32")]
    {
        None
    }
}

fn adapter_name(info: Option<&RenderAdapterInfo>) -> String {
    match info {
        Some(info) => format!("{} ({:?})", info.name, info.backend),
        None => String::from("unknown adapter"),
    }
}
