use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Result of looking up a model in the explicit alias registry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AliasResolution {
    Exact(String),
    Ambiguous,
    None,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "kind", content = "canonicalModel")]
enum AliasTarget {
    Exact(String),
    Ambiguous,
}

/// Exact model aliases accepted by the pricing boundary.
///
/// Keys and targets are normalized only for surrounding whitespace and ASCII
/// case. Prefix stripping, partial versions, and nearest-name matching belong
/// nowhere in this type: sources must register every accepted alias explicitly.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ModelAliasResolver {
    mappings: BTreeMap<String, AliasTarget>,
}

impl ModelAliasResolver {
    pub fn from_mappings<I, A, C>(mappings: I) -> Self
    where
        I: IntoIterator<Item = (A, C)>,
        A: AsRef<str>,
        C: AsRef<str>,
    {
        let mut resolver = Self::default();
        for (alias, canonical) in mappings {
            resolver.register(alias.as_ref(), canonical.as_ref());
        }
        resolver
    }

    pub fn register(&mut self, alias: &str, canonical_model: &str) {
        let alias = normalize_model_id(alias);
        let canonical_model = normalize_model_id(canonical_model);
        if alias.is_empty() || canonical_model.is_empty() {
            return;
        }

        match self.mappings.get(&alias) {
            None => {
                self.mappings
                    .insert(alias, AliasTarget::Exact(canonical_model));
            }
            Some(AliasTarget::Exact(existing)) if existing == &canonical_model => {}
            Some(_) => {
                self.mappings.insert(alias, AliasTarget::Ambiguous);
            }
        }
    }

    pub fn resolve_alias(&self, model: &str) -> AliasResolution {
        match self.mappings.get(&normalize_model_id(model)) {
            Some(AliasTarget::Exact(canonical)) => AliasResolution::Exact(canonical.clone()),
            Some(AliasTarget::Ambiguous) => AliasResolution::Ambiguous,
            None => AliasResolution::None,
        }
    }

    pub(crate) fn exact_targets(&self) -> impl Iterator<Item = &str> {
        self.mappings.values().filter_map(|target| match target {
            AliasTarget::Exact(canonical) => Some(canonical.as_str()),
            AliasTarget::Ambiguous => None,
        })
    }

    pub(crate) fn keys(&self) -> impl Iterator<Item = &str> {
        self.mappings.keys().map(String::as_str)
    }

    pub(crate) fn is_valid(&self) -> bool {
        self.mappings.iter().all(|(alias, target)| {
            !alias.is_empty()
                && alias == &normalize_model_id(alias)
                && match target {
                    AliasTarget::Exact(canonical) => {
                        !canonical.is_empty() && canonical == &normalize_model_id(canonical)
                    }
                    AliasTarget::Ambiguous => true,
                }
        })
    }
}

pub(crate) fn normalize_model_id(model: &str) -> String {
    model.trim().to_ascii_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_only_an_explicit_complete_alias() {
        let aliases = ModelAliasResolver::from_mappings([("gateway/gpt-test", "gpt-test")]);

        assert_eq!(
            aliases.resolve_alias("gateway/gpt-test"),
            AliasResolution::Exact("gpt-test".to_string())
        );
        assert_eq!(aliases.resolve_alias("gateway/gpt"), AliasResolution::None);
        assert_eq!(aliases.resolve_alias("gpt-test"), AliasResolution::None);
    }

    #[test]
    fn conflicting_explicit_mappings_are_ambiguous() {
        let aliases = ModelAliasResolver::from_mappings([
            ("gateway/shared", "gpt-test"),
            ("gateway/shared", "other-test"),
        ]);

        assert_eq!(
            aliases.resolve_alias("gateway/shared"),
            AliasResolution::Ambiguous
        );
    }

    #[test]
    fn normalization_is_limited_to_whitespace_and_ascii_case() {
        let aliases = ModelAliasResolver::from_mappings([("Gateway/GPT-Test", "GPT-Test")]);

        assert_eq!(
            aliases.resolve_alias(" gateway/gpt-test "),
            AliasResolution::Exact("gpt-test".to_string())
        );
        assert_eq!(
            aliases.resolve_alias("gateway/gpt-test-latest"),
            AliasResolution::None
        );
    }
}
