use crate::error::{Result, SlackersError};
use crate::slack::SlackClient;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Compact representation of a Slack user
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompactSlackUser {
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub real_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tz: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_bot: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deleted: Option<bool>,
}

/// List users in the workspace
///
/// Returns a vector of compact user representations.
/// Filters out bots unless include_bots is true.
pub async fn list_users(
    client: &SlackClient,
    limit: Option<usize>,
    include_bots: bool,
    mut on_page: Option<&mut dyn FnMut(&[CompactSlackUser])>,
) -> Result<Vec<CompactSlackUser>> {
    let mut all_users = Vec::new();
    let mut cursor: Option<String> = None;
    let effective_limit = limit.unwrap_or(usize::MAX);

    loop {
        let mut params = vec![("limit".to_string(), "200".to_string())];

        if let Some(c) = cursor {
            params.push(("cursor".to_string(), c));
        }

        let response = client.api_call("users.list", params).await?;

        let mut page_users = Vec::new();

        if let Some(members) = response.get("members").and_then(|v| v.as_array()) {
            for member in members {
                let is_bot = member
                    .get("is_bot")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);

                if is_bot && !include_bots {
                    continue;
                }

                let compact = to_compact_user(member);
                page_users.push(compact);

                if all_users.len() + page_users.len() >= effective_limit {
                    page_users.truncate(effective_limit - all_users.len());
                    if let Some(ref mut cb) = on_page {
                        cb(&page_users);
                    }
                    all_users.extend(page_users);
                    return Ok(all_users);
                }
            }
        }

        if let Some(ref mut cb) = on_page {
            if !page_users.is_empty() {
                cb(&page_users);
            }
        }
        all_users.extend(page_users);

        // Check for pagination
        cursor = response
            .get("response_metadata")
            .and_then(|m| m.get("next_cursor"))
            .and_then(|c| c.as_str())
            .filter(|c| !c.is_empty())
            .map(|c| c.to_string());

        if cursor.is_none() {
            break;
        }
    }

    Ok(all_users)
}

/// Get a specific user by ID or handle
///
/// If identifier starts with 'U', treats it as a user ID and calls users.info.
/// Otherwise, treats it as a handle (strips leading @) and searches by name.
pub async fn get_user(client: &SlackClient, identifier: &str) -> Result<CompactSlackUser> {
    let trimmed = identifier.trim();

    // If starts with U, it's a user ID
    if trimmed.starts_with('U') {
        let params = vec![("user".to_string(), trimmed.to_string())];
        let response = client.api_call("users.info", params).await?;

        let user = response
            .get("user")
            .ok_or_else(|| SlackersError::Other("No user in response".to_string()))?;

        return Ok(to_compact_user(user));
    }

    // Otherwise, search by handle (strip @ if present)
    let handle = trimmed.strip_prefix('@').unwrap_or(trimmed);

    // Search through all users
    let users = list_users(client, None, true, None).await?;

    for user_value in users {
        let matches = user_matches_handle(&user_value, handle);

        if matches {
            return Ok(user_value);
        }
    }

    Err(SlackersError::Other(format!(
        "User not found: {}",
        identifier
    )))
}

fn user_matches_handle(user: &CompactSlackUser, handle: &str) -> bool {
    user.name
        .as_deref()
        .map(|n| n.eq_ignore_ascii_case(handle))
        .unwrap_or(false)
        || user
            .display_name
            .as_deref()
            .map(|d| d.eq_ignore_ascii_case(handle))
            .unwrap_or(false)
        || user
            .real_name
            .as_deref()
            .map(|rn| rn.eq_ignore_ascii_case(handle))
            .unwrap_or(false)
}

/// Convert a full Slack user object to compact representation
fn to_compact_user(user: &Value) -> CompactSlackUser {
    let id = user
        .get("id")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let name = user.get("name").and_then(|v| v.as_str()).map(|s| s.to_string());

    let profile = user.get("profile");

    let real_name = profile
        .and_then(|p| p.get("real_name"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let display_name = profile
        .and_then(|p| p.get("display_name"))
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string());

    let email = profile
        .and_then(|p| p.get("email"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let title = profile
        .and_then(|p| p.get("title"))
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string());

    let tz = user.get("tz").and_then(|v| v.as_str()).map(|s| s.to_string());

    let is_bot = user.get("is_bot").and_then(|v| v.as_bool());

    let deleted = user.get("deleted").and_then(|v| v.as_bool());

    CompactSlackUser {
        id,
        name,
        real_name,
        display_name,
        email,
        title,
        tz,
        is_bot,
        deleted,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_to_compact_user_full() {
        let user = json!({
            "id": "U0123456789",
            "name": "john",
            "profile": {
                "real_name": "John Doe",
                "display_name": "johnd",
                "email": "john@example.com",
                "title": "Engineer"
            },
            "tz": "America/Los_Angeles",
            "is_bot": false,
            "deleted": false
        });

        let compact = to_compact_user(&user);

        assert_eq!(compact.id, "U0123456789");
        assert_eq!(compact.name, Some("john".to_string()));
        assert_eq!(compact.real_name, Some("John Doe".to_string()));
        assert_eq!(compact.display_name, Some("johnd".to_string()));
        assert_eq!(compact.email, Some("john@example.com".to_string()));
        assert_eq!(compact.title, Some("Engineer".to_string()));
        assert_eq!(compact.tz, Some("America/Los_Angeles".to_string()));
        assert_eq!(compact.is_bot, Some(false));
        assert_eq!(compact.deleted, Some(false));
    }

    #[test]
    fn test_to_compact_user_minimal() {
        let user = json!({
            "id": "U9876543210",
            "name": "bot",
            "is_bot": true
        });

        let compact = to_compact_user(&user);

        assert_eq!(compact.id, "U9876543210");
        assert_eq!(compact.name, Some("bot".to_string()));
        assert_eq!(compact.is_bot, Some(true));
        assert_eq!(compact.real_name, None);
        assert_eq!(compact.display_name, None);
        assert_eq!(compact.email, None);
    }

    #[test]
    fn test_user_matches_handle_case_insensitive_name() {
        let user = CompactSlackUser {
            id: "U123".to_string(),
            name: Some("John".to_string()),
            real_name: None,
            display_name: None,
            email: None,
            title: None,
            tz: None,
            is_bot: None,
            deleted: None,
        };
        assert!(user_matches_handle(&user, "john"));
        assert!(user_matches_handle(&user, "JOHN"));
        assert!(user_matches_handle(&user, "John"));
        assert!(!user_matches_handle(&user, "jane"));
    }

    #[test]
    fn test_user_matches_handle_case_insensitive_display_name() {
        let user = CompactSlackUser {
            id: "U456".to_string(),
            name: None,
            real_name: None,
            display_name: Some("JohnD".to_string()),
            email: None,
            title: None,
            tz: None,
            is_bot: None,
            deleted: None,
        };
        assert!(user_matches_handle(&user, "johnd"));
        assert!(user_matches_handle(&user, "JOHND"));
        assert!(user_matches_handle(&user, "JohnD"));
    }

    #[test]
    fn test_user_matches_handle_case_insensitive_real_name() {
        let user = CompactSlackUser {
            id: "U789".to_string(),
            name: None,
            real_name: Some("John Doe".to_string()),
            display_name: None,
            email: None,
            title: None,
            tz: None,
            is_bot: None,
            deleted: None,
        };
        assert!(user_matches_handle(&user, "john doe"));
        assert!(user_matches_handle(&user, "JOHN DOE"));
        assert!(user_matches_handle(&user, "John Doe"));
    }

    #[test]
    fn test_user_matches_handle_no_match() {
        let user = CompactSlackUser {
            id: "U000".to_string(),
            name: Some("alice".to_string()),
            real_name: Some("Alice Smith".to_string()),
            display_name: Some("alices".to_string()),
            email: None,
            title: None,
            tz: None,
            is_bot: None,
            deleted: None,
        };
        assert!(!user_matches_handle(&user, "bob"));
        assert!(!user_matches_handle(&user, "alic"));
    }

    #[test]
    fn test_to_compact_user_empty_strings() {
        let user = json!({
            "id": "U1111111111",
            "name": "user",
            "profile": {
                "display_name": "",
                "title": ""
            }
        });

        let compact = to_compact_user(&user);

        assert_eq!(compact.id, "U1111111111");
        assert_eq!(compact.name, Some("user".to_string()));
        // Empty strings should be filtered out
        assert_eq!(compact.display_name, None);
        assert_eq!(compact.title, None);
    }
}
