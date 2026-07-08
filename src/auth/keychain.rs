#[cfg(target_os = "macos")]
use std::process::Command;

/// Placeholder value written to credentials file when storing tokens in macOS Keychain
pub const KEYCHAIN_PLACEHOLDER: &str = "__KEYCHAIN__";

/// Service name used for all slackers keychain entries
pub const KEYCHAIN_SERVICE: &str = "slackers";

/// Retrieve a value from macOS Keychain
///
/// Uses: `security find-generic-password -w -a {account} -s {service}`
///
/// Returns None if the item doesn't exist or if not on macOS
pub fn keychain_get(account: &str, service: &str) -> Option<String> {
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (account, service);
        return None;
    }

    #[cfg(target_os = "macos")]
    {
        let output = Command::new("security")
            .args([
                "find-generic-password",
                "-w",  // only print password
                "-a", account,  // account name
                "-s", service,  // service name
            ])
            .output()
            .ok()?;

        if output.status.success() {
            String::from_utf8(output.stdout)
                .ok()
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
        } else {
            None
        }
    }
}

/// Store a value in macOS Keychain
///
/// Uses: `security add-generic-password -U -a {account} -s {service} -w {value}`
///
/// The `-U` flag updates if the entry already exists, creates otherwise.
/// Returns true on success, false on failure or if not on macOS.
pub fn keychain_set(account: &str, value: &str, service: &str) -> bool {
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (account, value, service);
        return false;
    }

    #[cfg(target_os = "macos")]
    {
        let status = Command::new("security")
            .args([
                "add-generic-password",
                "-U",  // update if exists
                "-a", account,
                "-s", service,
                "-w", value,
            ])
            .status()
            .ok();

        status.map(|s| s.success()).unwrap_or(false)
    }
}

/// Delete a value from macOS Keychain
///
/// Uses: `security delete-generic-password -a {account} -s {service}`
///
/// Returns true on success, false on failure or if not on macOS.
#[allow(dead_code)]
pub fn keychain_delete(account: &str, service: &str) -> bool {
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (account, service);
        return false;
    }

    #[cfg(target_os = "macos")]
    {
        let status = Command::new("security")
            .args([
                "delete-generic-password",
                "-a", account,
                "-s", service,
            ])
            .status()
            .ok();

        status.map(|s| s.success()).unwrap_or(false)
    }
}

/// Generate a keychain account name for a workspace URL and credential field
///
/// Format: "{workspace_url}:{field}"
/// Example: "https://myteam.slack.com:token"
pub fn keychain_account(workspace_url: &str, field: &str) -> String {
    format!("{}:{}", workspace_url, field)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_keychain_account() {
        assert_eq!(
            keychain_account("https://test.slack.com", "token"),
            "https://test.slack.com:token"
        );
    }

    #[test]
    #[cfg(target_os = "macos")]
    fn test_keychain_roundtrip() {
        let account = "slackers-test-account";
        let service = "slackers-test";
        let value = "test-secret-value";

        // Clean up any existing test entry
        keychain_delete(account, service);

        // Set value
        assert!(keychain_set(account, value, service));

        // Get value
        assert_eq!(keychain_get(account, service), Some(value.to_string()));

        // Clean up
        keychain_delete(account, service);

        // Verify deleted
        assert_eq!(keychain_get(account, service), None);
    }
}
