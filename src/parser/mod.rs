use anyhow::Result;
use std::path::PathBuf;

use crate::model::{FunctionInfo, Language};

mod rust;
mod python;
mod javascript;

pub fn parse_file(path: &PathBuf) -> Result<Vec<FunctionInfo>> {
    let lang = Language::from_path(path);
    let source = std::fs::read_to_string(path)?;

    match lang {
        Language::Rust => rust::parse(&source, path),
        Language::Python => python::parse(&source, path),
        Language::JavaScript | Language::TypeScript => javascript::parse(&source, path),
        Language::Unknown => {
            anyhow::bail!("Unsupported file type: {:?}", path.extension())
        }
    }
}
