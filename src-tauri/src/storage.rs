use std::{fs::File, path::Path};

use eyre::eyre;
use serde::Serialize;

fn write_atomic(
    path: &Path,
    write: impl FnOnce(&mut File) -> eyre::Result<()>,
) -> eyre::Result<()> {
    let directory = path.parent().ok_or_else(|| eyre!("文件缺少父目录"))?;
    let mut temporary = tempfile::NamedTempFile::new_in(directory)?;
    write(temporary.as_file_mut())?;
    temporary.persist(path).map_err(|error| error.error)?;
    Ok(())
}

pub fn write_json_atomic(path: &Path, value: &impl Serialize) -> eyre::Result<()> {
    write_atomic(path, |file| Ok(serde_json::to_writer(file, value)?))
}

pub fn write_pretty_json_atomic(path: &Path, value: &impl Serialize) -> eyre::Result<()> {
    write_atomic(path, |file| Ok(serde_json::to_writer_pretty(file, value)?))
}
