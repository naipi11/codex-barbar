//! Presentation-only account identity shared by desktop surfaces.

use serde::{Deserialize, Serialize};

use crate::accounts::avatar::AvatarKind;
use crate::accounts::identity::AccountStatus;
use crate::core::ProfileId;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PresentationIdentity {
    pub display_name: String,
    pub avatar_kind: AvatarKind,
    pub avatar_revision: Option<String>,
}

impl Default for PresentationIdentity {
    fn default() -> Self {
        presentation_identity(None, None, None, AccountStatus::Unavailable)
    }
}

pub fn presentation_identity(
    username: Option<&str>,
    display_name: Option<&str>,
    email: Option<&str>,
    status: AccountStatus,
) -> PresentationIdentity {
    let display_name = clean(username)
        .or_else(|| clean(display_name))
        .or_else(|| email.and_then(email_local_part))
        .unwrap_or_else(|| status_fallback(status).to_string());
    PresentationIdentity {
        display_name,
        avatar_kind: AvatarKind::Default,
        avatar_revision: None,
    }
}

pub fn avatar_asset_uri(profile_id: ProfileId, revision: &str) -> String {
    format!("account-avatar://profile/{profile_id}?rev={revision}")
}

fn clean(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn email_local_part(value: &str) -> Option<String> {
    let trimmed = value.trim();
    let (local, domain) = trimmed.split_once('@')?;
    if local.is_empty() || domain.is_empty() || domain.contains('@') {
        return None;
    }
    Some(local.to_string())
}

fn status_fallback(status: AccountStatus) -> &'static str {
    match status {
        AccountStatus::SignedIn => "已登录（名称不可用）",
        AccountStatus::SignedOut => "未登录",
        AccountStatus::Unavailable => "账号信息不可用",
    }
}

#[cfg(test)]
mod tests {
    use crate::accounts::avatar::AvatarKind;
    use crate::accounts::identity::AccountStatus;

    use super::presentation_identity;

    #[test]
    fn handle_precedes_display_name_and_email_local_part() {
        let identity = presentation_identity(
            Some("stack"),
            Some("Stack User"),
            Some("stack@example.com"),
            AccountStatus::SignedIn,
        );

        assert_eq!(identity.display_name, "stack");
        assert_eq!(identity.avatar_kind, AvatarKind::Default);
        assert_eq!(identity.avatar_revision, None);
    }

    #[test]
    fn display_name_precedes_email_local_part() {
        let identity = presentation_identity(
            None,
            Some("Stack User"),
            Some("stack@example.com"),
            AccountStatus::SignedIn,
        );

        assert_eq!(identity.display_name, "Stack User");
    }

    #[test]
    fn email_fallback_exposes_only_the_local_part() {
        let identity = presentation_identity(
            None,
            None,
            Some("stack@example.com"),
            AccountStatus::SignedIn,
        );

        assert_eq!(identity.display_name, "stack");
        assert!(!identity.display_name.contains('@'));
    }

    #[test]
    fn missing_identity_uses_a_nonempty_status_without_email_syntax() {
        for status in [
            AccountStatus::SignedIn,
            AccountStatus::SignedOut,
            AccountStatus::Unavailable,
        ] {
            let identity = presentation_identity(None, None, None, status);
            assert!(!identity.display_name.trim().is_empty());
            assert!(!identity.display_name.contains('@'));
        }
    }
}
