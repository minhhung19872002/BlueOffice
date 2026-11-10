//! Get & Transform data (SHEET-PQ-001) on the desktop: pick and read a folder, fetch a web
//! page. The engine never touches files or the network; the editor sends these bytes to it
//! (`querySource`).

use bo_sign::http::{HttpTransport, UreqTransport};
use serde::Serialize;
use tauri::ipc::Response;
use tauri_plugin_dialog::DialogExt;

/// Largest file of a folder source, and largest web page (bytes).
const MAX_BYTES: u64 = 20 * 1024 * 1024;
/// Most files read from one folder.
const MAX_FILES: usize = 1000;

/// A file of a folder.
#[derive(Serialize)]
pub struct FolderFile {
    name: String,
    /// Bytes, base64.
    data: String,
}

/// A folder read for a folder source.
#[derive(Serialize)]
pub struct Folder {
    name: String,
    path: String,
    files: Vec<FolderFile>,
}

/// The files directly in `path` (not its subfolders), by name.
fn read_dir(path: &std::path::Path) -> Result<Folder, String> {
    use base64::Engine as _;
    let mut files = Vec::new();
    let entries = std::fs::read_dir(path).map_err(|e| format!("{}: {e}", path.display()))?;
    for entry in entries.flatten() {
        let Ok(meta) = entry.metadata() else { continue };
        if !meta.is_file() || meta.len() > MAX_BYTES {
            continue;
        }
        let Ok(bytes) = std::fs::read(entry.path()) else {
            continue;
        };
        files.push(FolderFile {
            name: entry.file_name().to_string_lossy().into_owned(),
            data: base64::engine::general_purpose::STANDARD.encode(bytes),
        });
        if files.len() >= MAX_FILES {
            break;
        }
    }
    files.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(Folder {
        name: path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default(),
        path: path.to_string_lossy().into_owned(),
        files,
    })
}

/// Ask for a folder and read it (`None`: cancelled).
#[tauri::command]
pub async fn pick_folder(app: tauri::AppHandle) -> Result<Option<Folder>, String> {
    let Some(fp) = app.dialog().file().blocking_pick_folder() else {
        return Ok(None);
    };
    let path = fp.into_path().map_err(|e| e.to_string())?;
    read_dir(&path).map(Some)
}

/// Read a folder again by its path (refresh).
#[tauri::command]
pub async fn read_folder(path: String) -> Result<Folder, String> {
    read_dir(std::path::Path::new(&path))
}

/// GET a web page (http/https only, no credentials in the URL, 20 MB at most).
#[tauri::command]
pub async fn fetch_web(url: String) -> Result<Response, String> {
    let lower = url.to_ascii_lowercase();
    let authority = lower.split_once("://").map_or("", |(_, rest)| {
        rest.split(['/', '?', '#']).next().unwrap_or("")
    });
    if !(lower.starts_with("http://") || lower.starts_with("https://"))
        || authority.contains('@')
        || url.len() > 2048
    {
        return Err("network".into());
    }
    let resp = tauri::async_runtime::spawn_blocking(move || {
        UreqTransport::new(std::time::Duration::from_secs(30)).get(&url)
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|_| "network".to_owned())?;
    if !(200..300).contains(&resp.status) {
        return Err("status".into());
    }
    if resp.body.len() as u64 > MAX_BYTES {
        return Err("tooLarge".into());
    }
    Ok(Response::new(resp.body))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_files_of_a_folder() {
        let dir = std::env::temp_dir().join(format!("bo-query-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("sub")).unwrap();
        std::fs::write(dir.join("b.csv"), "x\n2\n").unwrap();
        std::fs::write(dir.join("a.csv"), "x\n1\n").unwrap();
        std::fs::write(dir.join("sub").join("c.csv"), "x\n3\n").unwrap();
        let f = read_dir(&dir).unwrap();
        let names: Vec<&str> = f.files.iter().map(|f| f.name.as_str()).collect();
        assert_eq!(names, ["a.csv", "b.csv"]);
        std::fs::remove_dir_all(&dir).unwrap();
        assert!(read_dir(&dir).is_err());
    }
}
