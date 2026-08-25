//! Safe, token-free identity hints from the local Codex auth cache.

use base64::Engine as _;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LocalIdentityHint {
    pub display_name: Option<String>,
    pub email: Option<String>,
    pub avatar_candidate: Option<String>,
}

pub fn identity_hint_from_auth_json(raw: &str) -> Option<LocalIdentityHint> {
    let root = serde_json::from_str::<serde_json::Value>(raw).ok()?;
    let tokens = root.get("tokens")?.as_object()?;
    let mut hint = LocalIdentityHint::default();

    for token_name in ["id_token", "access_token"] {
        let Some(token) = tokens.get(token_name).and_then(serde_json::Value::as_str) else {
            continue;
        };
        let Some(claims) = jwt_payload(token) else {
            continue;
        };
        if hint.email.is_none() {
            hint.email = claims.get("email").and_then(non_empty_string).or_else(|| {
                claims
                    .get("https://api.openai.com/profile")
                    .and_then(|profile| profile.get("email"))
                    .and_then(non_empty_string)
            });
        }
        if hint.display_name.is_none() {
            hint.display_name = claims.get("name").and_then(non_empty_string).or_else(|| {
                claims
                    .get("https://api.openai.com/profile")
                    .and_then(|profile| profile.get("name"))
                    .and_then(non_empty_string)
            });
        }
        if hint.avatar_candidate.is_none() {
            hint.avatar_candidate = [
                "picture",
                "pictureUrl",
                "avatarUrl",
                "avatar_url",
                "imageUrl",
            ]
            .into_iter()
            .find_map(|key| claims.get(key).and_then(non_empty_string))
            .or_else(|| {
                claims
                    .get("https://api.openai.com/profile")
                    .and_then(|profile| {
                        [
                            "picture",
                            "pictureUrl",
                            "avatarUrl",
                            "avatar_url",
                            "imageUrl",
                        ]
                        .into_iter()
                        .find_map(|key| profile.get(key).and_then(non_empty_string))
                    })
            });
        }
    }

    (hint.display_name.is_some() || hint.avatar_candidate.is_some()).then_some(hint)
}

fn jwt_payload(token: &str) -> Option<serde_json::Value> {
    let payload = token.split('.').nth(1)?;
    let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(payload)
        .or_else(|_| base64::engine::general_purpose::URL_SAFE.decode(payload))
        .ok()?;
    serde_json::from_slice(&bytes).ok()
}

fn non_empty_string(value: &serde_json::Value) -> Option<String> {
    value
        .as_str()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

#[cfg(test)]
mod tests {
    use super::identity_hint_from_auth_json;
    use base64::Engine as _;

    fn jwt(payload: serde_json::Value) -> String {
        let encoded = base64::engine::general_purpose::URL_SAFE_NO_PAD
            .encode(serde_json::to_vec(&payload).unwrap());
        format!("e.{encoded}.s")
    }

    #[test]
    fn extracts_display_name_from_id_token_without_returning_tokens() {
        let raw = serde_json::json!({
            "tokens": {
                "id_token": jwt(serde_json::json!({"name": "stack"}))
            }
        })
        .to_string();
        let hint = identity_hint_from_auth_json(&raw).expect("identity hint");
        assert_eq!(hint.display_name.as_deref(), Some("stack"));
        assert_eq!(hint.avatar_candidate, None);
    }

    #[test]
    fn extracts_profile_name_from_access_token_claim() {
        let raw = serde_json::json!({
            "tokens": {
                "access_token": jwt(serde_json::json!({
                    "https://api.openai.com/profile": {"name": "stack"}
                }))
            }
        })
        .to_string();
        let hint = identity_hint_from_auth_json(&raw).expect("identity hint");
        assert_eq!(hint.display_name.as_deref(), Some("stack"));
    }
}
