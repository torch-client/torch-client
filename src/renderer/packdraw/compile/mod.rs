use crate::shaderpack::directives::{self, Constants, Targets};
use crate::shaderpack::expressions::Customs;
use crate::shaderpack::options::{self, Values};
use crate::shaderpack::pipeline::Layout;
use crate::shaderpack::programs::{self, Geometry};
use crate::shaderpack::{backend, discover, features, include, pipeline, properties, transform};
use crate::util::pack::Source;

use super::images::{CustomTexture, CustomTextures};
use super::sources::TextureSource;
use super::uniforms::MatrixSet;
use crate::renderer::dimension::Dimension;

mod kinds;
mod pack;
mod preludes;
mod program;

pub(crate) use kinds::*;
use pack::Pack;
pub(crate) use pack::program_folder;
pub(crate) use preludes::{PACK_QUADS_SET, SCREEN_PRELUDE, terrain_prelude};
use program::{compile_compute, compile_program};

pub(crate) const PACK_SET: u32 = 0;

pub(crate) const TERRAIN_SET: u32 = 1;

const _: () = assert!(PACK_SET == 0 && TERRAIN_SET == 1);

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Dispatch {
    Fixed([u32; 3]),
    Screen { scale: [f32; 2], local: [u32; 2] },
}

impl Dispatch {
    pub(crate) fn groups(self, size: [u32; 2]) -> [u32; 3] {
        match self {
            Dispatch::Fixed(groups) => groups,
            Dispatch::Screen { scale, local } => {
                let axis = |n: usize| {
                    ((size[n] as f32 * scale[n]).ceil() / local[n] as f32)
                        .ceil()
                        .max(1.0) as u32
                };
                [axis(0), axis(1), 1]
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ProgramKind {
    Geometry,
    Screen,
    Shadow,
    Entity,
    ShadowEntity,
    Compute,
}

impl ProgramKind {
    pub(crate) fn render_stage(self) -> i32 {
        crate::shaderpack::features::render_stage(match self {
            ProgramKind::Geometry | ProgramKind::Shadow => "TERRAIN_SOLID",
            ProgramKind::Entity | ProgramKind::ShadowEntity => "ENTITIES",
            ProgramKind::Screen | ProgramKind::Compute => "NONE",
        })
    }

    pub(crate) fn matrices(self) -> MatrixSet {
        match self {
            ProgramKind::Geometry | ProgramKind::Entity | ProgramKind::Compute => MatrixSet::Camera,
            ProgramKind::Screen => MatrixSet::Screen,
            ProgramKind::Shadow | ProgramKind::ShadowEntity => MatrixSet::Shadow,
        }
    }

    pub(crate) fn is_shadow(self) -> bool {
        matches!(self, ProgramKind::Shadow | ProgramKind::ShadowEntity)
    }

    pub(crate) fn draws_entities(self) -> bool {
        matches!(self, ProgramKind::Entity | ProgramKind::ShadowEntity)
    }
}

pub(crate) struct Program {
    pub(crate) name: String,
    pub(crate) kind: ProgramKind,
    pub(crate) vertex: String,
    pub(crate) fragment: String,
    pub(crate) fragment_tests: Vec<(AlphaTest, String)>,
    pub(crate) alpha_override: Option<Option<AlphaTest>>,
    pub(crate) fragment_linear: Option<String>,
    pub(crate) compute: Option<String>,
    pub(crate) compute_used: Option<Vec<u32>>,
    pub(crate) compute_parity: Option<[(String, Vec<u32>); 2]>,
    pub(crate) dispatch: Dispatch,
    pub(crate) layout: Layout,
    pub(crate) draw_buffers: Vec<u8>,
    pub(crate) textures: Vec<(u32, TextureSource)>,
    pub(crate) mipmapped: u16,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ScreenStage {
    Begin,
    Prepare,
    Deferred,
    Composite,
}

impl ScreenStage {
    pub(crate) const ALL: [ScreenStage; 4] = [
        ScreenStage::Begin,
        ScreenStage::Prepare,
        ScreenStage::Deferred,
        ScreenStage::Composite,
    ];

    fn prefix(self) -> &'static str {
        match self {
            ScreenStage::Begin => "begin",
            ScreenStage::Prepare => "prepare",
            ScreenStage::Deferred => "deferred",
            ScreenStage::Composite => "composite",
        }
    }

    pub(crate) fn index(self) -> usize {
        self as usize
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Step {
    Compute(usize),
    Screen(usize),
}

pub(crate) struct Frame {
    pub(crate) programs: Vec<Program>,
    pub(crate) geometry: [Option<usize>; 3],
    pub(crate) stages: [Vec<Step>; 4],
    pub(crate) final_computes: Vec<usize>,
    pub(crate) final_pass: Option<usize>,
    pub(crate) setup: Vec<usize>,
    pub(crate) shadow: Option<usize>,
    pub(crate) shadowcomp: Vec<usize>,
    pub(crate) entities: [Option<usize>; EntityKind::COUNT],
    pub(crate) clouds_off: bool,
    pub(crate) sky_parts: SkyParts,
    pub(crate) shadow_casters: ShadowCasters,
    pub(crate) shadow_culling: ShadowCulling,
    pub(crate) targets: Targets,
    pub(crate) constants: Constants,
    pub(crate) customs: Customs,
    pub(crate) used_targets: u16,
    pub(crate) used_shadow: u8,
    pub(crate) images: Vec<CustomTexture>,
    pub(crate) block_ids: Option<std::sync::Arc<crate::renderer::packvertex::BlockIds>>,
    pub(crate) custom_images: Vec<crate::shaderpack::customimages::CustomImage>,
    pub(crate) buffers: Vec<crate::shaderpack::customimages::BufferObject>,
    pub(crate) entity_ids: std::collections::HashMap<String, i32>,
    pub(crate) item_ids: std::collections::HashMap<String, i32>,
    pub(crate) old_hand_light: bool,
    pub(crate) notes: Vec<String>,
}

impl Frame {
    fn draws_terrain(&self) -> bool {
        self.geometry.iter().any(Option::is_some)
    }

    fn push(
        &mut self,
        preprocessed: &mut Vec<String>,
        program: Program,
        sources: impl IntoIterator<Item = String>,
    ) -> usize {
        preprocessed.extend(sources);
        self.programs.push(program);
        self.programs.len() - 1
    }
}

pub(crate) fn compile_frame(
    name: &str,
    direct: bool,
    values: Option<Values>,
    dimension: Dimension,
) -> Result<Frame, String> {
    let source = Source::open(&discover::dir().join(name), crate::shaderpack::ROOT)?;
    let files: Vec<String> = crate::shaderpack::include::option_texts(&source);
    let declared = options::discover(files.iter().map(String::as_str));
    let values = values.unwrap_or_else(|| {
        std::fs::read_to_string(discover::dir().join(format!("{name}.txt")))
            .map(|text| Values::load(&text, &declared))
            .unwrap_or_default()
    });

    let props = source
        .read_text("/shaders.properties")
        .map(|text| properties::parse_with_options(&text, &declared, &values))
        .unwrap_or_default();
    let environment = features::program_defines(props.features("iris.features.optional"));
    let enable = programs::enable_expressions(props.other.iter())
        .map(|(p, e)| (p.to_owned(), e.to_owned()))
        .collect();
    let (customs, custom_problems) =
        Customs::compile(props.other.iter(), &super::uniforms::resolve_input);
    let mut notes: Vec<String> = custom_problems
        .into_iter()
        .map(|p| format!("custom uniform skipped: {p}"))
        .collect();
    let images = CustomTextures::read(&props.other, &source, &mut notes);
    let alpha_tests = alpha_tests(&props.other);
    let custom_images = crate::shaderpack::customimages::read(&props.other, &mut notes);
    let buffers = crate::shaderpack::customimages::buffers(&props.other, &mut notes);

    let folder = program_folder(&source, dimension);
    let mut pack = Pack {
        source,
        declared,
        values,
        environment,
        enable,
        cache: include::Cache::new(),
        images,
        alpha_tests,
        folder,
        custom_images,
        buffers,
    };
    let pack_lighting = crate::renderer::packvertex::PackLighting {
        old_lighting: true_key(&props.other, "oldLighting", false),
        separate_ao: true_key(&props.other, "separateAo", false),
    };
    let mut frame = Frame {
        programs: Vec::new(),
        geometry: [None; 3],
        stages: Default::default(),
        final_computes: Vec::new(),
        final_pass: None,
        setup: Vec::new(),
        shadow: None,
        shadowcomp: Vec::new(),
        entities: [None; EntityKind::COUNT],
        clouds_off: clouds_off(&props.other),
        sky_parts: SkyParts::read(&props.other),
        shadow_casters: ShadowCasters::read(&props.other),
        shadow_culling: ShadowCulling::parse(props.other.get("shadow.culling").map(String::as_str)),
        targets: Targets::default(),
        constants: Constants::default(),
        customs,
        used_targets: 0,
        used_shadow: 0,
        images: Vec::new(),
        block_ids: None,
        custom_images: Vec::new(),
        buffers: Vec::new(),
        entity_ids: std::collections::HashMap::new(),
        item_ids: std::collections::HashMap::new(),
        old_hand_light: false_key(&props.other, "oldHandLight"),
        notes,
    };
    let mut preprocessed: Vec<String> = Vec::new();

    let resolved: Vec<Option<&'static str>> = Geometry::ALL
        .iter()
        .map(|g| programs::resolve(*g, |n| pack.has(n)))
        .collect();
    for (slot, program) in resolved.iter().enumerate() {
        let Some(program) = *program else { continue };
        let index = match frame.programs.iter().position(|p| p.name == program) {
            Some(index) => index,
            None => {
                let tests: Vec<AlphaTest> = Geometry::ALL
                    .iter()
                    .zip(&resolved)
                    .filter(|(_, p)| **p == Some(program))
                    .filter_map(|(g, _)| {
                        alpha_for(
                            pack.alpha_tests.get(program).copied(),
                            geometry_alpha(*g, false),
                        )
                    })
                    .collect();
                let (compiled, sources) =
                    compile_program(&mut pack, program, ProgramKind::Geometry, &tests, direct)?;
                frame.push(&mut preprocessed, compiled, sources)
            }
        };
        frame.geometry[slot] = Some(index);
    }

    fn computes(
        pack: &mut Pack,
        frame: &mut Frame,
        preprocessed: &mut Vec<String>,
        names: Vec<String>,
    ) -> Vec<usize> {
        let mut out = Vec::new();
        for name in names {
            match compile_compute(pack, &name) {
                Ok((compiled, source)) => out.push(frame.push(preprocessed, compiled, [source])),
                Err(e) => frame.notes.push(format!("{name} skipped: {e}")),
            }
        }
        out
    }
    let numbered = |prefix: &str, n: u32| {
        if n == 0 {
            prefix.to_owned()
        } else {
            format!("{prefix}{n}")
        }
    };
    for stage in ScreenStage::ALL {
        for n in 0..=programs::LAST_PASS {
            let name = numbered(stage.prefix(), n);
            let names = pack.compute_array(&name);
            let indices = computes(&mut pack, &mut frame, &mut preprocessed, names);
            frame.stages[stage.index()].extend(indices.into_iter().map(Step::Compute));
            if pack.has(&name) {
                let (compiled, sources) =
                    compile_program(&mut pack, &name, ProgramKind::Screen, &[], direct)?;
                let index = frame.push(&mut preprocessed, compiled, sources);
                frame.stages[stage.index()].push(Step::Screen(index));
            }
        }
    }
    let names = pack.compute_array("final");
    frame.final_computes = computes(&mut pack, &mut frame, &mut preprocessed, names);
    if pack.has("final") {
        let (compiled, sources) =
            compile_program(&mut pack, "final", ProgramKind::Screen, &[], direct)?;
        frame.final_pass = Some(frame.push(&mut preprocessed, compiled, sources));
    }
    let names = (0..=programs::LAST_PASS)
        .map(|n| numbered("setup", n))
        .filter(|n| pack.has_compute(n))
        .collect();
    frame.setup = computes(&mut pack, &mut frame, &mut preprocessed, names);

    if frame.programs.is_empty() {
        return Err("the pack has no program this client draws with".into());
    }

    if frame.draws_terrain() {
        let mut kinds: Vec<(EntityKind, &'static str)> = Vec::new();
        for kind in EntityKind::ALL {
            let skipped = match kind {
                EntityKind::ShadowCaster => true,
                EntityKind::Clouds => frame.clouds_off,
                EntityKind::SkyBasic => !frame.sky_parts.sky && !frame.sky_parts.stars,
                EntityKind::SkyTextured => !frame.sky_parts.sun && !frame.sky_parts.moon,
                _ => false,
            };
            if skipped {
                continue;
            }
            if let Some(name) = kind.resolve(|n| pack.has(n)) {
                kinds.push((kind, name));
            }
        }
        let mut names: Vec<&'static str> = kinds.iter().map(|(_, n)| *n).collect();
        names.sort_unstable();
        names.dedup();
        for name in names {
            let program_override = pack.alpha_tests.get(name).copied();
            let tests: Vec<AlphaTest> = kinds
                .iter()
                .filter(|(_, n)| *n == name)
                .flat_map(|(kind, _)| kind.alpha_tests())
                .filter_map(|default| alpha_for(program_override, default))
                .collect();
            match compile_program(&mut pack, name, ProgramKind::Entity, &tests, direct) {
                Ok((compiled, sources)) => {
                    let index = frame.push(&mut preprocessed, compiled, sources);
                    for (kind, _) in kinds.iter().filter(|(_, n)| *n == name) {
                        frame.entities[kind.index()] = Some(index);
                    }
                }
                Err(e) => frame
                    .notes
                    .push(format!("no entity program: {name} did not compile: {e}")),
            }
        }
    }

    if frame.draws_terrain() && pack.has("shadow") {
        let program_override = pack.alpha_tests.get("shadow").copied();
        let tests: Vec<AlphaTest> = Geometry::ALL
            .iter()
            .filter_map(|g| alpha_for(program_override, geometry_alpha(*g, true)))
            .collect();
        match compile_program(&mut pack, "shadow", ProgramKind::Shadow, &tests, direct) {
            Ok((compiled, sources)) => {
                frame.shadow = Some(frame.push(&mut preprocessed, compiled, sources))
            }
            Err(e) => frame.notes.push(format!(
                "no shadows: the shadow program did not compile: {e}"
            )),
        }
    }

    if frame.shadow.is_some()
        && frame.shadow_casters.any()
        && let Some(name) = EntityKind::ShadowCaster.resolve(|n| pack.has(n))
    {
        let program_override = pack.alpha_tests.get(name).copied();
        let tests: Vec<AlphaTest> = EntityKind::ShadowCaster
            .alpha_tests()
            .into_iter()
            .filter_map(|default| alpha_for(program_override, default))
            .collect();
        match compile_program(&mut pack, name, ProgramKind::ShadowEntity, &tests, direct) {
            Ok((compiled, sources)) => {
                frame.entities[EntityKind::ShadowCaster.index()] =
                    Some(frame.push(&mut preprocessed, compiled, sources));
            }
            Err(e) => frame.notes.push(format!(
                "no entity shadows: {name} did not compile for entities: {e}"
            )),
        }
    }

    if frame.shadow.is_some() {
        for n in 0..=programs::LAST_PASS {
            let names = pack.compute_array(&numbered("shadowcomp", n));
            let indices = computes(&mut pack, &mut frame, &mut preprocessed, names);
            frame.shadowcomp.extend(indices);
        }
    }

    let (mut targets, unknown) = directives::read_targets(preprocessed.iter().map(String::as_str));
    directives::read_sizes(&mut targets, props.other.iter(), &mut frame.notes);
    let size_notes = hold_sizes(&mut targets, &frame.programs, frame.final_pass)?;
    frame.notes.extend(size_notes);
    for program in &frame.programs {
        for buffer in &program.draw_buffers {
            let settings = if program.kind.is_shadow() {
                &targets.shadow[*buffer as usize]
            } else {
                &targets.settings[*buffer as usize]
            };
            let kind = settings.format.sample_kind();
            if matches!(
                kind,
                directives::SampleKind::Uint | directives::SampleKind::Sint
            ) && program.name != "final"
            {
                return Err(format!(
                    "{} draws to target {buffer}, an integer format, which is not supported yet",
                    program.name
                ));
            }
        }
    }
    frame.targets = targets;
    frame.constants = directives::read_constants(preprocessed.iter().map(String::as_str));
    frame.notes.extend(
        unknown
            .into_iter()
            .map(|u| format!("unknown target format: {u}")),
    );

    let mut used = 1u16;
    let mut used_shadow = 0u8;
    for (index, program) in frame.programs.iter().enumerate() {
        for buffer in &program.draw_buffers {
            if program.kind.is_shadow() {
                used_shadow |= 1u8 << buffer;
            } else if Some(index) != frame.final_pass {
                used |= 1u16 << buffer;
            }
        }
        for (_, source) in &program.textures {
            match source {
                TextureSource::Color(n) => used |= 1u16 << n,
                TextureSource::ShadowColor(n) => used_shadow |= 1u8 << n,
                _ => {}
            }
        }
    }
    frame.used_targets = used;
    frame.used_shadow = used_shadow;
    frame.images = std::mem::take(&mut pack.images.textures);
    frame.custom_images = std::mem::take(&mut pack.custom_images);
    frame.buffers = std::mem::take(&mut pack.buffers);
    frame.entity_ids = pack.id_map(
        "/entity.properties",
        crate::shaderpack::blockids::entity_entries,
        &mut frame.notes,
    );
    frame.item_ids = pack.id_map(
        "/item.properties",
        crate::shaderpack::blockids::item_entries,
        &mut frame.notes,
    );
    if frame.draws_terrain() {
        let entries = pack
            .source
            .read_text("/block.properties")
            .map(|text| {
                let props = properties::parse_with_options(&text, &pack.declared, &pack.values);
                crate::shaderpack::blockids::entries(&props.other, &mut frame.notes)
            })
            .unwrap_or_default();
        frame.block_ids = Some(std::sync::Arc::new(
            crate::renderer::packvertex::BlockIds::build(&entries, pack_lighting),
        ));
    }
    Ok(frame)
}

fn hold_sizes(
    targets: &mut Targets,
    programs: &[Program],
    final_pass: Option<usize>,
) -> Result<Vec<String>, String> {
    let screen_sized = programs
        .iter()
        .filter(|p| matches!(p.kind, ProgramKind::Geometry | ProgramKind::Entity))
        .flat_map(|p| &p.draw_buffers)
        .fold(1u16, |mask, b| mask | 1 << b);
    let mut notes = Vec::new();
    for (n, settings) in targets.settings.iter_mut().enumerate() {
        if screen_sized & (1 << n) != 0 && settings.size.take().is_some() {
            notes.push(format!("size.buffer.colortex{n} ignored: the gbuffers or the screen use it at the screen's size"));
        }
    }
    for (index, program) in programs.iter().enumerate() {
        if program.kind != ProgramKind::Screen || Some(index) == final_pass {
            continue;
        }
        let mut sizes = program
            .draw_buffers
            .iter()
            .map(|b| targets.settings[usize::from(*b)].size);
        if let Some(first) = sizes.next()
            && sizes.any(|size| size != first)
        {
            return Err(format!(
                "{}: its draw buffers {:?} differ in size",
                program.name, program.draw_buffers
            ));
        }
    }
    Ok(notes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shaderpack::transform::Stage;

    #[test]
    fn a_compute_block_is_sized_as_the_shader_declares_it() {
        let csh = "#version 430\nlayout(local_size_x = 8) in;\nuniform float rainStrength;\nuniform vec3 cameraPosition;\n\
                   layout(r32f) uniform image2D img;\n\
                   void main() { imageStore(img, ivec2(gl_GlobalInvocationID.xy), vec4(rainStrength + cameraPosition.x)); }\n";
        let shared = transform::Shared::of_compute(csh);
        let target = transform::Target {
            set: PACK_SET,
            image_formats: &[("img", "r32f")],
            ..Default::default()
        };
        let out = transform::transform(csh, Stage::Compute, &shared, &target);
        let layout = pipeline::describe_program(pipeline::Program::Compute(&out.interface));
        let wgsl = backend::to_wgsl(&out.source, backend::ShaderStage::Compute, &[])
            .unwrap_or_else(|e| panic!("{e}\n\n{}", out.source));
        let module = naga::front::wgsl::parse_str(&wgsl).expect("parses");
        let mut layouter = naga::proc::Layouter::default();
        layouter.update(module.to_ctx()).expect("lays out");
        let block = module
            .global_variables
            .iter()
            .find(|(_, v)| v.space == naga::AddressSpace::Uniform)
            .expect("a uniform block")
            .1;
        assert_eq!(layout.uniforms.size, layouter[block.ty].size, "{wgsl}");
        assert!(
            layout
                .uniforms
                .members
                .iter()
                .any(|m| m.name == "rainStrength")
        );
    }

    #[test]
    fn screen_dispatch_covers_the_scaled_screen() {
        let dispatch = Dispatch::Screen {
            scale: [0.5, 1.0],
            local: [8, 8],
        };
        assert_eq!(dispatch.groups([1920, 1080]), [120, 135, 1]);
        assert_eq!(dispatch.groups([1921, 1]), [121, 1, 1]);
        assert_eq!(Dispatch::Fixed([16, 8, 16]).groups([1, 1]), [16, 8, 16]);
    }

    #[test]
    #[ignore = "needs a shader pack installed"]
    fn solas_shadowcomp_splits_by_parity() {
        let Some(pack) = discover::list()
            .into_iter()
            .find(|p| p.name == "Solas Shader V3.7b")
        else {
            return;
        };
        let frame = compile_frame(&pack.name, false, None, Dimension::Overworld).expect("compiles");
        let shadowcomp = frame
            .programs
            .iter()
            .find(|p| p.name == "shadowcomp")
            .expect("has shadowcomp");
        let [(_, even), (_, odd)] = shadowcomp.compute_parity.as_ref().expect("split by parity");
        println!("even {even:?}, odd {odd:?}");
        assert_ne!(even, odd);
    }

    fn drawing(name: &str, kind: ProgramKind, draw_buffers: &[u8]) -> Program {
        Program {
            name: name.to_owned(),
            kind,
            vertex: String::new(),
            fragment: String::new(),
            fragment_tests: Vec::new(),
            alpha_override: None,
            fragment_linear: None,
            compute: None,
            compute_used: None,
            compute_parity: None,
            dispatch: Dispatch::Fixed([0; 3]),
            layout: Layout::default(),
            draw_buffers: draw_buffers.to_vec(),
            textures: Vec::new(),
            mipmapped: 0,
        }
    }

    fn sized(keys: &[(&str, &str)]) -> Targets {
        let keys: Vec<(String, String)> = keys
            .iter()
            .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
            .collect();
        let mut targets = Targets::default();
        directives::read_sizes(
            &mut targets,
            keys.iter().map(|(k, v)| (k, v)),
            &mut Vec::new(),
        );
        targets
    }

    #[test]
    fn only_screen_pass_targets_keep_their_size() {
        let mut targets = sized(&[
            ("size.buffer.colortex0", "0.5 0.5"),
            ("size.buffer.colortex2", "0.5 0.5"),
            ("size.buffer.colortex6", "0.33 0.33"),
        ]);
        let programs = [
            drawing("gbuffers_terrain", ProgramKind::Geometry, &[0, 2]),
            drawing("composite1", ProgramKind::Screen, &[6]),
        ];
        let notes = hold_sizes(&mut targets, &programs, None).expect("allowed");
        assert_eq!(notes.len(), 2, "{notes:?}");
        assert!(targets.settings[0].size.is_none() && targets.settings[2].size.is_none());
        assert_eq!(targets.settings[6].resolve_size([900, 900]), [297, 297]);
    }

    #[test]
    fn a_pass_drawing_targets_of_two_sizes_is_refused() {
        let mut targets = sized(&[("size.buffer.colortex6", "0.5 0.5")]);
        let programs = [drawing("composite1", ProgramKind::Screen, &[6, 7])];
        assert!(hold_sizes(&mut targets, &programs, None).is_err());
    }

    #[test]
    #[ignore = "needs a shader pack installed"]
    fn pegasus_scales_its_background_and_skips_flood_fill_copies() {
        let Some(pack) = discover::list()
            .into_iter()
            .find(|p| p.name.starts_with("Pegasus"))
        else {
            return;
        };
        let frame = compile_frame(&pack.name, false, None, Dimension::Overworld).expect("compiles");
        assert_eq!(
            frame.targets.settings[6].resolve_size([1920, 1002]),
            [633, 330]
        );
        assert_eq!(
            frame.targets.settings[9].resolve_size([1920, 1002]),
            [192, 100]
        );
        for program in frame
            .programs
            .iter()
            .filter(|p| p.name.starts_with("prepare"))
        {
            let used = program.compute_used.as_ref().expect("bindings told");
            let unread: Vec<u32> = program
                .textures
                .iter()
                .map(|(b, _)| *b)
                .filter(|b| !used.contains(b))
                .collect();
            println!(
                "{}: uses {used:?}, unread samplers {unread:?}",
                program.name
            );
            assert_eq!(
                unread.len(),
                program.textures.len(),
                "{} samples an image it writes",
                program.name
            );
        }
    }

    #[test]
    #[ignore = "needs a shader pack installed"]
    fn every_installed_program_translates() {
        for pack in discover::list() {
            for direct in [false, true] {
                match compile_frame(&pack.name, direct, None, Dimension::Overworld) {
                    Ok(frame) => {
                        println!(
                            "ok    {} (direct={direct}): {} programs",
                            pack.name,
                            frame.programs.len()
                        );
                        for program in &frame.programs {
                            println!(
                                "      {:<24} {:?} draw {:?}, {} samplers, zero: {}",
                                program.name,
                                program.kind,
                                program.draw_buffers,
                                program.textures.len(),
                                super::super::uniforms::unsupplied(
                                    &program.layout.uniforms,
                                    &frame.customs
                                )
                                .len()
                            );
                        }
                        println!(
                            "      geometry {:?}, stages {:?}, final {:?}, targets {:016b}",
                            frame.geometry, frame.stages, frame.final_pass, frame.used_targets
                        );
                        for note in &frame.notes {
                            println!("      note: {note}");
                        }
                        let mut zero: Vec<String> = frame
                            .programs
                            .iter()
                            .flat_map(|p| {
                                super::super::uniforms::unsupplied(
                                    &p.layout.uniforms,
                                    &frame.customs,
                                )
                            })
                            .collect();
                        zero.sort();
                        zero.dedup();
                        println!("      reading zero ({}): {}", zero.len(), zero.join(", "));
                    }
                    Err(e) => println!("FAIL  {} (direct={direct}): {e}", pack.name),
                }
            }
        }
    }
}
