use super::super::literal;
use super::lexer::{self, Kind, Token};
use super::{Interface, Shared, Target, first_image_binding};

const MEMORY_QUALIFIERS: &[&str] = &["coherent", "volatile", "restrict"];

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct StorageBuffer {
    pub(crate) index: u8,
    pub(crate) binding: u32,
    pub(crate) read_only: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Image {
    pub(crate) name: String,
    pub(crate) ty: String,
    pub(crate) binding: u32,
    pub(crate) access: ImageAccess,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ImageAccess {
    Write,
    Read,
    ReadWrite,
}

impl ImageAccess {
    pub(crate) fn union(self, other: ImageAccess) -> ImageAccess {
        if self == other {
            self
        } else {
            ImageAccess::ReadWrite
        }
    }
}

fn image_access(tokens: &[Token], at: usize) -> ImageAccess {
    let mut access = ImageAccess::ReadWrite;
    let mut depth = 0i32;
    for token in tokens[..at].iter().rev().filter(|t| !t.is_trivia()) {
        match token.text.as_str() {
            ")" if token.kind == Kind::Punct => depth += 1,
            "(" if token.kind == Kind::Punct => depth -= 1,
            _ if depth > 0 => {}
            "writeonly" => access = ImageAccess::Write,
            "readonly" => access = ImageAccess::Read,
            "layout" => {}
            other if MEMORY_QUALIFIERS.contains(&other) => {}
            _ => break,
        }
    }
    access
}

pub(crate) fn untyped(ty: &str) -> &str {
    match ty.strip_prefix(['i', 'u']) {
        Some(rest)
            if ["image", "sampler", "texture"]
                .iter()
                .any(|p| rest.starts_with(p)) =>
        {
            rest
        }
        _ => ty,
    }
}

pub(super) fn image_type(ty: &str) -> bool {
    untyped(ty).strip_prefix("image").is_some_and(|shape| {
        matches!(
            shape,
            "1D" | "2D" | "3D" | "Cube" | "1DArray" | "2DArray" | "CubeArray" | "2DRect" | "Buffer"
        )
    })
}

pub(super) fn image_names(tokens: &[Token]) -> Vec<String> {
    let mut names = Vec::new();
    for (_, ty, name) in lexer::uniform_declarations(tokens) {
        if image_type(&tokens[ty].text) && !names.contains(&tokens[name].text) {
            names.push(tokens[name].text.clone());
        }
    }
    names
}

pub(super) fn bind_images(
    tokens: &mut [Token],
    interface: &mut Interface,
    shared: &Shared,
    target: &Target,
) {
    let first = first_image_binding(shared);
    let declared: Vec<(usize, usize, usize)> = lexer::uniform_declarations(tokens).collect();
    for (at, ty, name) in declared {
        if !image_type(&tokens[ty].text) {
            continue;
        }
        let Some((_, format)) = target
            .image_formats
            .iter()
            .find(|(n, _)| *n == tokens[name].text)
        else {
            continue;
        };
        let index = shared
            .images
            .iter()
            .position(|n| *n == tokens[name].text)
            .unwrap_or(shared.images.len() + interface.images.len()) as u32;
        let binding = first + index;
        let access = image_access(tokens, at);
        tokens[at].text = format!(
            "layout(set = {}, binding = {binding}, {format}) uniform",
            target.set
        );
        interface.images.push(Image {
            name: tokens[name].text.clone(),
            ty: tokens[ty].text.clone(),
            binding,
            access,
        });
    }
}

fn first_buffer_binding(shared: &Shared) -> u32 {
    first_image_binding(shared) + shared.images.len() as u32
}

pub(super) fn bind_buffers(
    tokens: &mut [Token],
    interface: &mut Interface,
    shared: &Shared,
    target: &Target,
) {
    let first = first_buffer_binding(shared);
    for at in 0..tokens.len() {
        if !tokens[at].is_word("layout") {
            continue;
        }
        let Some(open) = lexer::next_solid(tokens, at + 1).filter(|o| tokens[*o].is_punct('('))
        else {
            continue;
        };
        let Some(close) = lexer::matching(tokens, open) else {
            continue;
        };
        let mut read_only = false;
        let mut is_buffer = false;
        let mut next = lexer::next_solid(tokens, close + 1);
        while let Some(i) = next {
            match tokens[i].text.as_str() {
                "buffer" => {
                    is_buffer = true;
                    break;
                }
                "readonly" => read_only = true,
                "writeonly" => {}
                other if MEMORY_QUALIFIERS.contains(&other) => {}
                _ => break,
            }
            next = lexer::next_solid(tokens, i + 1);
        }
        if !is_buffer {
            continue;
        }
        let inside: Vec<usize> = (open + 1..close)
            .filter(|i| !tokens[*i].is_trivia())
            .collect();
        let Some(index) = inside
            .windows(3)
            .find(|w| tokens[w[0]].is_word("binding") && tokens[w[1]].is_punct('='))
            .and_then(|w| literal::int(&tokens[w[2]].text).and_then(|n| u8::try_from(n).ok()))
        else {
            continue;
        };
        let binding = first + u32::from(index);
        let layout: Vec<String> = inside
            .split(|i| tokens[*i].is_punct(','))
            .filter(|part| {
                !part
                    .first()
                    .is_some_and(|i| tokens[*i].is_word("binding") || tokens[*i].is_word("set"))
            })
            .map(|part| {
                part.iter()
                    .map(|i| tokens[*i].text.as_str())
                    .collect::<Vec<_>>()
                    .join(" ")
            })
            .filter(|p| !p.is_empty())
            .collect();
        let mut rewritten = format!(
            "layout({}set = {}, binding = {binding})",
            layout.iter().map(|p| format!("{p}, ")).collect::<String>(),
            target.set
        );
        rewritten.push(' ');
        tokens[at].text = rewritten;
        for token in &mut tokens[at + 1..=close] {
            token.text.clear();
        }
        interface.buffers.push(StorageBuffer {
            index,
            binding,
            read_only,
        });
    }
}
