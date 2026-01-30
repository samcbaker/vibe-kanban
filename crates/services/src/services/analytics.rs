// Telemetry removed - this module is now a no-op stub

use serde_json::Value;

#[derive(Debug, Clone)]
pub struct AnalyticsContext {
    pub user_id: String,
    pub analytics_service: AnalyticsService,
}

#[derive(Debug, Clone)]
pub struct AnalyticsConfig {
    pub posthog_api_key: String,
    pub posthog_api_endpoint: String,
}

impl AnalyticsConfig {
    /// Always returns None - analytics has been disabled
    pub fn new() -> Option<Self> {
        None
    }
}

#[derive(Clone, Debug)]
pub struct AnalyticsService {
    _private: (),
}

impl AnalyticsService {
    pub fn new(_config: AnalyticsConfig) -> Self {
        Self { _private: () }
    }

    /// No-op: analytics has been disabled
    pub fn track_event(&self, _user_id: &str, _event_name: &str, _properties: Option<Value>) {
        // No-op - telemetry disabled
    }
}

/// Generates a consistent, anonymous user ID.
/// Returns a hex string prefixed with "npm_user_"
pub fn generate_user_id() -> String {
    use std::{
        collections::hash_map::DefaultHasher,
        hash::{Hash, Hasher},
    };

    let mut hasher = DefaultHasher::new();

    #[cfg(target_os = "macos")]
    {
        // Use ioreg to get hardware UUID
        if let Ok(output) = std::process::Command::new("ioreg")
            .args(["-rd1", "-c", "IOPlatformExpertDevice"])
            .output()
        {
            let stdout = String::from_utf8_lossy(&output.stdout);
            if let Some(line) = stdout.lines().find(|l| l.contains("IOPlatformUUID")) {
                line.hash(&mut hasher);
            }
        }
    }

    #[cfg(target_os = "linux")]
    {
        if let Ok(machine_id) = std::fs::read_to_string("/etc/machine-id") {
            machine_id.trim().hash(&mut hasher);
        }
    }

    #[cfg(target_os = "windows")]
    {
        // Use PowerShell to get machine GUID from registry
        if let Ok(output) = std::process::Command::new("powershell")
            .args(&[
                "-NoProfile",
                "-Command",
                "(Get-ItemProperty -Path 'HKLM:\\SOFTWARE\\Microsoft\\Cryptography').MachineGuid",
            ])
            .output()
        {
            if output.status.success() {
                output.stdout.hash(&mut hasher);
            }
        }
    }

    // Add username for per-user differentiation
    if let Ok(user) = std::env::var("USER").or_else(|_| std::env::var("USERNAME")) {
        user.hash(&mut hasher);
    }

    // Add home directory for additional entropy
    if let Ok(home) = std::env::var("HOME").or_else(|_| std::env::var("USERPROFILE")) {
        home.hash(&mut hasher);
    }

    format!("npm_user_{:016x}", hasher.finish())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_user_id_format() {
        let id = generate_user_id();
        assert!(id.starts_with("npm_user_"));
        assert_eq!(id.len(), 25);
    }

    #[test]
    fn test_consistency() {
        let id1 = generate_user_id();
        let id2 = generate_user_id();
        assert_eq!(id1, id2, "ID should be consistent across calls");
    }

    #[test]
    fn test_analytics_config_returns_none() {
        assert!(AnalyticsConfig::new().is_none());
    }
}
