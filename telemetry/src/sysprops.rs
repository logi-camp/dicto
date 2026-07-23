//! System property collection — the non-sensitive device context attached to
//! every event as Aptabase `systemProps`.
//!
//! We collect: OS name + version, app version, system locale, and the SDK
//! version string. We deliberately do NOT collect `deviceModel` (the machine
//! hostname) since that can de-anonymize an installation.

use serde::Serialize;

/// Aptabase `systemProps`. All fields are server-enriched/required by the
/// ingestion API or dashboard.
#[derive(Debug, Serialize)]
pub struct SystemProps {
    #[serde(rename = "osName")]
    pub os_name: String,
    #[serde(rename = "osVersion")]
    pub os_version: String,
    #[serde(rename = "appVersion")]
    pub app_version: String,
    pub locale: String,
    #[serde(rename = "sdkVersion")]
    pub sdk_version: String,
}

pub fn sdk_version() -> &'static str {
    "dicto-telemetry@0.1.0"
}

/// Build system props for this session. Called once at client construction.
pub fn collect(app_version: String) -> SystemProps {
    SystemProps {
        os_name: os_name(),
        os_version: os_version(),
        app_version,
        locale: locale(),
        sdk_version: sdk_version().to_string(),
    }
}

fn os_name() -> String {
    if cfg!(target_os = "windows") {
        "Windows".to_string()
    } else if cfg!(target_os = "macos") {
        "macOS".to_string()
    } else if cfg!(target_os = "linux") {
        "Linux".to_string()
    } else {
        std::env::consts::OS.to_string()
    }
}

fn os_version() -> String {
    // Best-effort, prefer no crash over precision.
    if let Some(v) = read_os_version() {
        return v;
    }
    String::new()
}

#[cfg(target_os = "linux")]
fn read_os_version() -> Option<String> {
    // /etc/os-release PRETTY_NAME gives a useful distro string; fall back to
    // the kernel version from uname.
    if let Ok(contents) = std::fs::read_to_string("/etc/os-release") {
        for line in contents.lines() {
            if let Some(rest) = line.strip_prefix("PRETTY_NAME=") {
                return Some(rest.trim_matches('"').to_string());
            }
        }
    }
    // kernel version as a last resort
    std::process::Command::new("uname")
        .arg("-r")
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
}

#[cfg(target_os = "windows")]
fn read_os_version() -> Option<String> {
    // RtlGetVersion / registry would be more reliable, but a simple
    // cmd `ver` is enough granularity. Strip the leading "Microsoft Windows
    // [" / trailing "]".
    std::process::Command::new("cmd")
        .args(["/C", "ver"])
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
}

#[cfg(target_os = "macos")]
fn read_os_version() -> Option<String> {
    std::process::Command::new("sw_vers")
        .arg("-productVersion")
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
}

#[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
fn read_os_version() -> Option<String> {
    None
}

fn locale() -> String {
    // Prefer standard env vars; these are non-sensitive language/region tags.
    for key in ["LANG", "LC_ALL", "LC_MESSAGES"] {
        if let Ok(val) = std::env::var(key) {
            let cleaned = val.split('.').next().unwrap_or(&val);
            if !cleaned.is_empty() {
                return cleaned.to_string();
            }
        }
    }
    String::new()
}
