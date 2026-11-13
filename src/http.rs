//! HTTP over ureq, with one identity and one error shape for the whole app.

use std::io::Read;
use std::path::Path;
use std::time::Duration;

pub const USER_AGENT: &str = "ExxaTools/1.0 (Linux; +https://github.com/Hintay/vapor-forge)";
const TIMEOUT: Duration = Duration::from_secs(20);

#[derive(Debug, Clone)]
pub struct HttpError {
    pub message: String,
    pub status: Option<u16>,
}

impl HttpError {
    fn new(message: impl Into<String>) -> Self {
        Self { message: message.into(), status: None }
    }
}

impl std::fmt::Display for HttpError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.status {
            Some(status) => write!(f, "{} (HTTP {status})", self.message),
            None => f.write_str(&self.message),
        }
    }
}

impl std::error::Error for HttpError {}

impl From<ureq::Error> for HttpError {
    fn from(error: ureq::Error) -> Self {
        match error {
            ureq::Error::StatusCode(code) => Self { message: format!("HTTP {code}"), status: Some(code) },
            other => Self::new(other.to_string()),
        }
    }
}

impl From<std::io::Error> for HttpError {
    fn from(error: std::io::Error) -> Self {
        Self::new(error.to_string())
    }
}

fn agent_for(headers: Option<&str>) -> String {
    headers.unwrap_or(USER_AGENT).to_string()
}

/// Used by the CDP target endpoint, which answers a new target only on PUT.
pub fn put(url: &str) -> Result<Vec<u8>, HttpError> {
    let config = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(10)))
        .user_agent(USER_AGENT)
        .build();
    let agent: ureq::Agent = config.into();
    let mut response = agent.put(url).send_empty().map_err(HttpError::from)?;
    let mut body = Vec::new();
    response
        .body_mut()
        .as_reader()
        .read_to_end(&mut body)
        .map_err(HttpError::from)?;
    Ok(body)
}

pub fn get_with(url: &str, user_agent: Option<&str>, timeout: Option<Duration>) -> Result<Vec<u8>, HttpError> {
    let config = ureq::Agent::config_builder()
        .timeout_global(Some(timeout.unwrap_or(TIMEOUT)))
        .user_agent(agent_for(user_agent))
        .build();
    let agent: ureq::Agent = config.into();
    let mut response = agent.get(url).call().map_err(HttpError::from)?;
    let mut body = Vec::new();
    response
        .body_mut()
        .as_reader()
        .read_to_end(&mut body)
        .map_err(HttpError::from)?;
    Ok(body)
}

/// Same as `get_with`, with request headers. Used for token-authenticated plan
/// sources that refuse anonymous requests.
pub fn get_with_headers(url: &str, headers: &[(&str, &str)]) -> Result<Vec<u8>, HttpError> {
    let config = ureq::Agent::config_builder()
        .timeout_global(Some(TIMEOUT))
        .user_agent(USER_AGENT)
        .build();
    let agent: ureq::Agent = config.into();
    let mut request = agent.get(url);
    for (name, value) in headers {
        request = request.header(*name, *value);
    }
    let mut response = request.call().map_err(HttpError::from)?;
    let mut body = Vec::new();
    response
        .body_mut()
        .as_reader()
        .read_to_end(&mut body)
        .map_err(HttpError::from)?;
    Ok(body)
}

pub fn get_text_with(url: &str, user_agent: Option<&str>, timeout: Option<Duration>) -> Result<String, HttpError> {
    let body = get_with(url, user_agent, timeout)?;
    Ok(String::from_utf8_lossy(&body).into_owned())
}

pub fn get_json(url: &str) -> Result<serde_json::Value, HttpError> {
    get_json_with(url, None)
}

pub fn get_json_with(url: &str, user_agent: Option<&str>) -> Result<serde_json::Value, HttpError> {
    let text = get_text_with(url, user_agent, None)?;
    serde_json::from_str(&text).map_err(|error| HttpError {
        message: format!("{url} returned non-JSON payload: {error}"),
        status: None,
    })
}

/// Streams a URL to disk and returns (sha256, bytes).
pub fn download(url: &str, target: &Path) -> Result<(String, u64), HttpError> {
    use sha2::{Digest, Sha256};

    if let Some(parent) = target.parent() {
        std::fs::create_dir_all(parent).map_err(HttpError::from)?;
    }
    let config = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(120)))
        .user_agent(USER_AGENT)
        .build();
    let agent: ureq::Agent = config.into();
    let mut response = agent.get(url).call().map_err(HttpError::from)?;
    let mut reader = response.body_mut().as_reader();
    let temp = target.with_extension("part");
    let mut file = std::fs::File::create(&temp).map_err(HttpError::from)?;
    let mut hasher = Sha256::new();
    let mut size = 0u64;
    let mut buffer = vec![0u8; 1 << 20];
    loop {
        let read = reader.read(&mut buffer).map_err(HttpError::from)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
        std::io::Write::write_all(&mut file, &buffer[..read]).map_err(HttpError::from)?;
        size += read as u64;
    }
    drop(file);
    std::fs::rename(&temp, target).map_err(HttpError::from)?;
    Ok((format!("{:x}", hasher.finalize()), size))
}

pub fn sha256_file(path: &Path) -> Result<String, HttpError> {
    use sha2::{Digest, Sha256};

    let mut file = std::fs::File::open(path).map_err(HttpError::from)?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0u8; 1 << 20];
    loop {
        let read = file.read(&mut buffer).map_err(HttpError::from)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

pub fn quote(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char)
            }
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}