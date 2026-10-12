//! Tauri commands exposed to the webview, with TypeScript types generated
//! from the Rust definitions (`ts-rs`) — single source of truth for both
//! sides of the bridge.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Static application information served to the UI at startup.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../ui/src/types/generated/")]
pub struct AppInfo {
    /// Application version from the crate manifest.
    pub version: String,
    /// Machine-friendly app name.
    pub name: String,
}

impl AppInfo {
    /// Builds the info from compile-time crate metadata.
    #[must_use]
    pub fn current() -> Self {
        Self {
            version: env!("CARGO_PKG_VERSION").to_string(),
            name: "AI Juggler".to_string(),
        }
    }
}

/// Returns static application information.
#[tauri::command]
#[must_use]
pub fn app_info() -> AppInfo {
    AppInfo::current()
}

#[cfg(test)]
mod tests {
    use super::AppInfo;

    #[test]
    fn app_info_is_populated() {
        let info = AppInfo::current();
        assert_eq!(info.name, "AI Juggler");
        assert!(!info.version.is_empty());
    }
}
