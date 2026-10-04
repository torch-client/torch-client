pub(crate) const ROOT: &str = "shaders";

pub(crate) const SOURCE_EXTENSIONS: &[&str] = &[".glsl", ".vsh", ".fsh", ".gsh", ".csh"];

pub(crate) mod backend;
pub(crate) mod blockids;
pub(crate) mod customimages;
pub(crate) mod directives;
pub(crate) mod discover;
pub(crate) mod expressions;
pub(crate) mod features;
pub(crate) mod include;
pub(crate) mod lang;
pub(crate) mod literal;
pub(crate) mod options;
pub(crate) mod pipeline;
pub(crate) mod preprocess;
pub(crate) mod programs;
pub(crate) mod properties;
pub(crate) mod transform;
