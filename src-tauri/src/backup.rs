use crate::db;
use anyhow::{Context, Result};
use std::fs::File;
use std::io::{Read, Write};
use std::path::Path;
use walkdir::WalkDir;
use zip::write::SimpleFileOptions;
use zip::CompressionMethod;

pub fn export_library_zip(dest: &str) -> Result<String> {
    let root = db::app_data_dir();
    if !root.exists() {
        anyhow::bail!("Library data folder does not exist yet");
    }
    let dest_path = Path::new(dest);
    if let Some(parent) = dest_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let file = File::create(dest_path).context("create backup zip")?;
    let mut zip = zip::ZipWriter::new(file);
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);

    let root_canon = root.canonicalize().unwrap_or(root.clone());
    for entry in WalkDir::new(&root).into_iter().filter_map(|e| e.ok()) {
        let path = entry.path();
        if path.is_dir() {
            continue;
        }
        let rel = path
            .strip_prefix(&root_canon)
            .or_else(|_| path.strip_prefix(&root))
            .unwrap_or(path);
        let name = rel.to_string_lossy().replace('\\', "/");
        if name.is_empty() {
            continue;
        }
        zip.start_file(name, options)?;
        let mut f = File::open(path)?;
        let mut buf = Vec::new();
        f.read_to_end(&mut buf)?;
        zip.write_all(&buf)?;
    }
    zip.finish()?;
    Ok(dest_path.to_string_lossy().to_string())
}
