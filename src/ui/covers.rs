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

/// The header cover if it is already cached; never touches the network.
pub fn cached_cover(appid: u64) -> Option<PathBuf> {
    let path = cover_path(appid);
    std::fs::metadata(&path)
        .is_ok_and(|meta| meta.is_file() && meta.len() > 1024)
        .then_some(path)
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

/// Steam library art kept next to the header covers. Each kind has a fixed
/// path on the public CDN; a title that lacks one gets a short-lived marker so
/// Discover does not re-request sixty missing files on every load.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Asset {
    /// Wide, logo-free key art behind the inspector and the window backdrop.
    Hero,
    /// The 600x900 portrait poster.
    Capsule,
    /// Transparent title logo drawn over the hero.
    Logo,
}

/// A missing asset is retried after this long; Steam adds library art to
/// older titles from time to time.
const MISSING_TTL: std::time::Duration = std::time::Duration::from_secs(7 * 24 * 60 * 60);

impl Asset {
    fn file(self, appid: u64) -> String {
        match self {
            Self::Hero => format!("{appid}-hero.jpg"),
            Self::Capsule => format!("{appid}-capsule.jpg"),
            Self::Logo => format!("{appid}-logo.png"),
        }
    }

    fn remote(self) -> &'static str {
        match self {
            Self::Hero => "library_hero.jpg",
            Self::Capsule => "library_600x900.jpg",
            Self::Logo => "logo.png",
        }
    }

    /// The store API's name for the same art, for titles whose generic CDN
    /// path does not exist. Logos have no store counterpart.
    fn store_key(self) -> Option<&'static str> {
        match self {
            Self::Hero => Some("library_hero"),
            Self::Capsule => Some("library_capsule_2x"),
            Self::Logo => None,
        }
    }

    /// Anything smaller is an error page or a placeholder, not art.
    fn min_len(self) -> usize {
        match self {
            Self::Hero | Self::Capsule => 2048,
            Self::Logo => 512,
        }
    }
}

fn missing_is_fresh(age: std::time::Duration) -> bool {
    age < MISSING_TTL
}

/// The asset if it is already cached; never touches the network.
pub fn cached_asset(appid: u64, asset: Asset) -> Option<PathBuf> {
    let path = cover_dir().join(asset.file(appid));
    path.is_file().then_some(path)
}

/// Returns the local file for `asset`, downloading it once. Only a definite
/// "not there" answer (HTTP 403/404) is remembered; network errors are not.
pub fn ensure_asset(appid: u64, asset: Asset) -> Option<PathBuf> {
    if appid == 0 {
        return None;
    }
    let dir = cover_dir();
    let name = asset.file(appid);
    let path = dir.join(&name);
    if path.is_file() {
        return Some(path);
    }
    let marker = dir.join(format!("{name}.missing"));
    let marked = std::fs::metadata(&marker)
        .and_then(|meta| meta.modified())
        .ok()
        .and_then(|modified| modified.elapsed().ok())
        .is_some_and(missing_is_fresh);
    if marked {
        return None;
    }
    std::fs::create_dir_all(&dir).ok()?;
    let generic = format!(
        "https://cdn.akamai.steamstatic.com/steam/apps/{appid}/{}",
        asset.remote()
    );
    let mut fetched = fetch(&generic, asset.min_len());
    // Newer titles keep library art under hashed store paths only; the
    // store API names the file when the generic CDN path is absent.
    if matches!(fetched, Fetch::Missing) {
        if let Some(url) = asset.store_key().and_then(|key| store_asset_url(appid, key)) {
            fetched = fetch(&url, asset.min_len());
        }
    }
    let body = match fetched {
        Fetch::Body(body) => body,
        Fetch::Missing => {
            let _ = std::fs::write(&marker, b"");
            return None;
        }
        Fetch::Failed => return None,
    };
    let temp = dir.join(format!("{name}.tmp"));
    std::fs::write(&temp, body).ok()?;
    std::fs::rename(&temp, &path).ok()?;
    let _ = std::fs::remove_file(&marker);
    Some(path)
}

enum Fetch {
    Body(Vec<u8>),
    /// Steam answered: the file is not there (403/404 or a stub body).
    Missing,
    /// Network trouble; worth retrying next time.
    Failed,
}

fn fetch(url: &str, min_len: usize) -> Fetch {
    match crate::http::get_with(url, None, Some(std::time::Duration::from_secs(7))) {
        Ok(body) if body.len() >= min_len => Fetch::Body(body),
        Ok(_) => Fetch::Missing,
        Err(error) if matches!(error.status, Some(403 | 404)) => Fetch::Missing,
        Err(_) => Fetch::Failed,
    }
}

/// Asks IStoreBrowseService for the hashed path of one library asset.
fn store_asset_url(appid: u64, key: &str) -> Option<String> {
    let input = serde_json::json!({
        "ids": [{ "appid": appid }],
        "context": { "language": "english", "country_code": "US" },
        "data_request": { "include_assets": true },
    });
    let url = format!(
        "https://api.steampowered.com/IStoreBrowseService/GetItems/v1?input_json={}",
        query_escape(&input.to_string())
    );
    let body = crate::http::get_with(&url, None, Some(std::time::Duration::from_secs(7))).ok()?;
    let value: serde_json::Value = serde_json::from_slice(&body).ok()?;
    store_asset_from_response(appid, &value, key)
}

fn store_asset_from_response(appid: u64, value: &serde_json::Value, key: &str) -> Option<String> {
    let file = value
        .pointer("/response/store_items/0/assets")?
        .get(key)?
        .as_str()?;
    if file.is_empty() || file.contains("..") || file.starts_with('/') {
        return None;
    }
    Some(format!(
        "https://shared.akamai.steamstatic.com/store_item_assets/steam/apps/{appid}/{file}"
    ))
}

fn query_escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len() * 3);
    for byte in text.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~') {
            out.push(byte as char);
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
    out
}

/// The wide Steam library art used behind the inspector's title.
pub fn ensure_hero(appid: u64) -> Option<PathBuf> {
    ensure_asset(appid, Asset::Hero)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn asset_files_never_collide_with_the_header_cover() {
        let header = format!("{}.jpg", 620);
        let files = [Asset::Hero, Asset::Capsule, Asset::Logo].map(|asset| asset.file(620));
        assert!(!files.contains(&header));
        assert_eq!(files, ["620-hero.jpg", "620-capsule.jpg", "620-logo.png"]);
    }

    #[test]
    fn existing_hero_cache_keeps_its_name() {
        assert_eq!(Asset::Hero.file(108600), "108600-hero.jpg");
        assert_eq!(Asset::Hero.remote(), "library_hero.jpg");
    }

    #[test]
    fn a_missing_marker_expires_after_a_week() {
        assert!(missing_is_fresh(Duration::from_secs(60)));
        assert!(missing_is_fresh(Duration::from_secs(6 * 24 * 60 * 60)));
        assert!(!missing_is_fresh(Duration::from_secs(8 * 24 * 60 * 60)));
    }

    #[test]
    fn store_paths_come_from_the_asset_record_only() {
        let response = serde_json::json!({"response": {"store_items": [{
            "appid": 2075800,
            "assets": {
                "library_capsule_2x": "b31b/library_capsule_2x.jpg",
                "library_hero": "../escape.jpg"
            }
        }]}});
        assert_eq!(
            store_asset_from_response(2075800, &response, "library_capsule_2x").as_deref(),
            Some("https://shared.akamai.steamstatic.com/store_item_assets/steam/apps/2075800/b31b/library_capsule_2x.jpg")
        );
        assert_eq!(store_asset_from_response(2075800, &response, "library_hero"), None);
        assert_eq!(store_asset_from_response(2075800, &response, "header"), None);
    }

    #[test]
    fn query_escape_encodes_json_punctuation() {
        assert_eq!(query_escape(r#"{"a":[1]}"#), "%7B%22a%22%3A%5B1%5D%7D");
    }

    #[test]
    #[ignore = "network: hits the Steam CDN"]
    fn steam_serves_portrait_and_logo_art() {
        for appid in [620, 108600, 2605790] {
            assert!(ensure_asset(appid, Asset::Capsule).is_some(), "capsule {appid}");
            assert!(ensure_asset(appid, Asset::Logo).is_some(), "logo {appid}");
        }
        // Only reachable through the store API's hashed path.
        let marker = cover_dir().join(format!("{}.missing", Asset::Capsule.file(2075800)));
        let _ = std::fs::remove_file(marker);
        assert!(ensure_asset(2075800, Asset::Capsule).is_some(), "hashed capsule");
    }
}
