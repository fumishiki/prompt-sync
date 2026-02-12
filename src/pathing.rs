use std::borrow::Cow;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use crate::model::ResolveContext;

#[cfg(unix)]
use std::os::unix::fs::MetadataExt;

pub(crate) struct PathTemplate<'a> {
    raw: &'a str,
}

impl<'a> PathTemplate<'a> {
    pub(crate) const fn new(raw: &'a str) -> Self {
        Self { raw }
    }

    pub(crate) fn resolve(&self, ctx: &ResolveContext) -> PathBuf {
        let with_tokens = substitute_tokens(Cow::Borrowed(self.raw), ctx);
        if let Some(home) = &ctx.home_dir
            && (with_tokens == "~" || with_tokens.starts_with("~/"))
        {
            let suffix = with_tokens.trim_start_matches('~').trim_start_matches('/');
            let mut path = home.clone();
            if !suffix.is_empty() {
                path.push(suffix);
            }
            return path;
        }

        let path = PathBuf::from(with_tokens.as_ref());
        if path.is_absolute() {
            path
        } else {
            ctx.config_dir.join(path)
        }
    }
}

pub(crate) fn resolve_path(raw: &str, ctx: &ResolveContext) -> PathBuf {
    PathTemplate::new(raw).resolve(ctx)
}

fn substitute_tokens<'a>(input: Cow<'a, str>, ctx: &ResolveContext) -> Cow<'a, str> {
    let input = replace_token(input, "<repo>", &ctx.repo_root_text);

    if let Some(home_text) = &ctx.home_dir_text {
        replace_token(input, "<home>", home_text)
    } else {
        input
    }
}

fn replace_token<'a>(input: Cow<'a, str>, token: &str, replacement: &str) -> Cow<'a, str> {
    if input.contains(token) {
        Cow::Owned(input.replace(token, replacement))
    } else {
        input
    }
}

pub(crate) fn absolute_path(path: &Path) -> Result<PathBuf> {
    if path.is_absolute() {
        return Ok(path.to_path_buf());
    }
    let cwd = env::current_dir().context("failed to resolve current directory")?;
    Ok(cwd.join(path))
}

#[cfg(unix)]
pub(crate) fn same_file(a: &fs::Metadata, b: &fs::Metadata) -> bool {
    a.ino() == b.ino() && a.dev() == b.dev()
}

#[cfg(not(unix))]
pub(crate) fn same_file(a: &fs::Metadata, b: &fs::Metadata) -> bool {
    a.len() == b.len()
}

#[cfg(unix)]
pub(crate) fn hardlink_count(meta: &fs::Metadata) -> u64 {
    meta.nlink()
}

#[cfg(not(unix))]
pub(crate) fn hardlink_count(_meta: &fs::Metadata) -> u64 {
    1
}
