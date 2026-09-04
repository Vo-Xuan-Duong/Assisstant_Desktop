use std::{
    fs,
    path::{Path, PathBuf},
};

use serde::Serialize;

use crate::{ToolError, ToolResult};

const DEFAULT_LIST_LIMIT: usize = 100;
const MAX_LIST_LIMIT: usize = 500;

#[derive(Debug, Clone, Serialize)]
pub struct FileEntry {
    pub path: String,
    pub name: String,
    pub kind: &'static str,
    pub size_bytes: Option<u64>,
    pub readonly: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct FileListResult {
    pub directory: String,
    pub entries: Vec<FileEntry>,
    pub truncated: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct FileMutationResult {
    pub action: &'static str,
    pub source: Option<String>,
    pub destination: Option<String>,
    pub ok: bool,
}

fn absolute_path(raw: &str, field: &str) -> ToolResult<PathBuf> {
    let raw = raw.trim();
    if raw.is_empty() {
        return Err(ToolError::InvalidArgument(format!("{field} cannot be empty")));
    }
    let path = PathBuf::from(raw);
    if !path.is_absolute() {
        return Err(ToolError::InvalidArgument(format!(
            "{field} must be an absolute Windows path"
        )));
    }
    Ok(path)
}

fn entry_from_path(path: &Path) -> ToolResult<FileEntry> {
    let metadata = fs::symlink_metadata(path)?;
    let file_type = metadata.file_type();
    let kind = if file_type.is_symlink() {
        "symlink"
    } else if file_type.is_dir() {
        "directory"
    } else if file_type.is_file() {
        "file"
    } else {
        "other"
    };
    Ok(FileEntry {
        path: path.display().to_string(),
        name: path
            .file_name()
            .map(|value| value.to_string_lossy().into_owned())
            .unwrap_or_else(|| path.display().to_string()),
        kind,
        size_bytes: file_type.is_file().then_some(metadata.len()),
        readonly: metadata.permissions().readonly(),
    })
}

pub fn info(path: &str) -> ToolResult<FileEntry> {
    let path = absolute_path(path, "path")?;
    if !path.exists() {
        return Err(ToolError::NotFound(format!("{} does not exist", path.display())));
    }
    entry_from_path(&path)
}

pub fn list(directory: &str, max_entries: Option<usize>) -> ToolResult<FileListResult> {
    let directory = absolute_path(directory, "directory")?;
    if !directory.is_dir() {
        return Err(ToolError::NotFound(format!(
            "{} is not an existing directory",
            directory.display()
        )));
    }

    let limit = max_entries.unwrap_or(DEFAULT_LIST_LIMIT).clamp(1, MAX_LIST_LIMIT);
    let mut entries = Vec::new();
    let mut truncated = false;
    for item in fs::read_dir(&directory)? {
        let item = item?;
        if entries.len() == limit {
            truncated = true;
            break;
        }
        entries.push(entry_from_path(&item.path())?);
    }
    entries.sort_unstable_by(|left, right| {
        left.name
            .to_ascii_lowercase()
            .cmp(&right.name.to_ascii_lowercase())
    });

    Ok(FileListResult {
        directory: directory.display().to_string(),
        entries,
        truncated,
    })
}

pub fn create_directory(path: &str) -> ToolResult<FileMutationResult> {
    let path = absolute_path(path, "path")?;
    if path.exists() {
        return Err(ToolError::Unsupported(format!(
            "{} already exists",
            path.display()
        )));
    }
    fs::create_dir_all(&path)?;
    Ok(FileMutationResult {
        action: "create_directory",
        source: None,
        destination: Some(path.display().to_string()),
        ok: true,
    })
}

pub fn copy_file(source: &str, destination: &str) -> ToolResult<FileMutationResult> {
    let source = absolute_path(source, "source")?;
    let destination = absolute_path(destination, "destination")?;
    let metadata = fs::symlink_metadata(&source)?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(ToolError::Unsupported(
            "file_copy only accepts a regular file source; directory/symlink copy is not exposed".into(),
        ));
    }
    if destination.exists() {
        return Err(ToolError::Unsupported(format!(
            "destination {} already exists; overwrite is intentionally disabled",
            destination.display()
        )));
    }
    if let Some(parent) = destination.parent() {
        if !parent.is_dir() {
            return Err(ToolError::NotFound(format!(
                "destination parent {} does not exist",
                parent.display()
            )));
        }
    }
    fs::copy(&source, &destination)?;
    Ok(FileMutationResult {
        action: "copy_file",
        source: Some(source.display().to_string()),
        destination: Some(destination.display().to_string()),
        ok: true,
    })
}

pub fn move_path(source: &str, destination: &str) -> ToolResult<FileMutationResult> {
    let source = absolute_path(source, "source")?;
    let destination = absolute_path(destination, "destination")?;
    if source.parent().is_none() {
        return Err(ToolError::Unsupported(
            "moving a filesystem root is never allowed".into(),
        ));
    }
    let metadata = fs::symlink_metadata(&source)?;
    if metadata.file_type().is_symlink() {
        return Err(ToolError::Unsupported(
            "moving symlinks/junctions is intentionally not exposed".into(),
        ));
    }
    if destination.exists() {
        return Err(ToolError::Unsupported(format!(
            "destination {} already exists; overwrite is intentionally disabled",
            destination.display()
        )));
    }
    if let Some(parent) = destination.parent() {
        if !parent.is_dir() {
            return Err(ToolError::NotFound(format!(
                "destination parent {} does not exist",
                parent.display()
            )));
        }
    }
    fs::rename(&source, &destination)?;
    Ok(FileMutationResult {
        action: "move",
        source: Some(source.display().to_string()),
        destination: Some(destination.display().to_string()),
        ok: true,
    })
}

pub fn delete_path(path: &str) -> ToolResult<FileMutationResult> {
    let path = absolute_path(path, "path")?;
    if path.parent().is_none() {
        return Err(ToolError::Unsupported(
            "deleting a filesystem root is never allowed".into(),
        ));
    }
    let metadata = fs::symlink_metadata(&path)?;
    if metadata.file_type().is_symlink() {
        return Err(ToolError::Unsupported(
            "deleting symlinks/junctions is intentionally not exposed".into(),
        ));
    }

    if metadata.is_file() {
        fs::remove_file(&path)?;
    } else if metadata.is_dir() {
        // Deliberately non-recursive: non-empty directories fail rather than
        // allowing the model to delete an arbitrary tree in one request.
        fs::remove_dir(&path)?;
    } else {
        return Err(ToolError::Unsupported(
            "only regular files and empty directories can be deleted".into(),
        ));
    }

    Ok(FileMutationResult {
        action: "delete",
        source: Some(path.display().to_string()),
        destination: None,
        ok: true,
    })
}
