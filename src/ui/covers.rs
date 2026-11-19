//! Steam header art, cached on disk so a second launch has no network at all.

use std::path::PathBuf;

pub fn cover_dir() -> PathBuf {
    crate::paths::state_dir().join("covers")
}

pub fn cover_path(appid: u64) -> PathBuf {
    cover_dir().join(format!("{appid}.jpg"))
}

/// Returns a local cover path, downloading once when the cache is cold.
/// A network failure is not an error: the UI falls back to a monogram.
pub fn ensure_cover(appid: u64) -> Option<PathBuf> {
    if appid == 0 {
        return None;
    }
    let path = cover_path(appid);
    if path.is_file()
        && std::fs::metadata(&path)
            .map(|meta| meta.len() > 1024)
            .unwrap_or(false)
    {
        return Some(path);
    }
    let _ = std::fs::remove_file(&path);
    let urls = [
        format!("https://cdn.akamai.steamstatic.com/steam/apps/{appid}/header.jpg"),
        format!("https://cdn.akamai.steamstatic.com/steam/apps/{appid}/capsule_231x87.jpg"),
    ];
    for url in urls {
        if let Some(path) = download(appid, &url) {
            return Some(path);
        }
    }
    None
}

/// Downloads a store-provided capsule URL, which is the only reliable image
/// for apps whose generic CDN path was renamed.
pub fn ensure_cover_url(appid: u64, url: &str) -> Option<PathBuf> {
    if appid == 0 || !url.starts_with("https://") {
        return None;
    }
    let path = cover_path(appid);
    if path.is_file()
        && std::fs::metadata(&path)
            .map(|meta| meta.len() > 1024)
            .unwrap_or(false)
    {
        return Some(path);
    }
    let _ = std::fs::remove_file(&path);
    download(appid, url)
}

fn download(appid: u64, url: &str) -> Option<PathBuf> {
    std::fs::create_dir_all(cover_dir()).ok()?;
    let body = crate::http::get_with(url, None, Some(std::time::Duration::from_secs(7))).ok()?;
    if body.len() < 1024 {
        return None;
    }
    let path = cover_path(appid);
    let temp = cover_dir().join(format!("{appid}.jpg.tmp"));
    std::fs::write(&temp, body).ok()?;
    std::fs::rename(&temp, &path).ok()?;
    Some(path)
}

/// The wide Steam library art used behind the inspector's title.
pub fn ensure_hero(appid: u64) -> Option<PathBuf> {
    if appid == 0 {
        return None;
    }
    let path = cover_dir().join(format!("{appid}-hero.jpg"));
    if path.is_file() {
        return Some(path);
    }
    std::fs::create_dir_all(cover_dir()).ok()?;
    let url = format!("https://cdn.akamai.steamstatic.com/steam/apps/{appid}/library_hero.jpg");
    let body = crate::http::get_with(&url, None, Some(std::time::Duration::from_secs(7))).ok()?;
    if body.len() < 2048 {
        return None;
    }
    let temp = cover_dir().join(format!("{appid}-hero.jpg.tmp"));
    std::fs::write(&temp, body).ok()?;
    std::fs::rename(&temp, &path).ok()?;
    Some(path)
}
