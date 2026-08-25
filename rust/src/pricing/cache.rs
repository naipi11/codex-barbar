use super::catalog::{CatalogValidationError, PricingCatalog};
#[cfg(unix)]
use std::fs::File;
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use thiserror::Error;
use uuid::Uuid;

const CATALOG_FILE_NAME: &str = "pricing-catalog-v1.json";

#[derive(Debug, Error)]
pub enum CatalogStoreError {
    #[error("unable to read or write the pricing catalog cache: {0}")]
    Io(#[from] io::Error),
    #[error("unable to encode the pricing catalog cache: {0}")]
    Encode(serde_json::Error),
    #[error("unable to decode the pricing catalog cache: {0}")]
    Decode(serde_json::Error),
    #[error("pricing catalog cache is incomplete: {0}")]
    IncompleteCatalog(#[from] CatalogValidationError),
}

#[derive(Debug, Clone)]
pub struct CatalogStore {
    path: PathBuf,
}

impl CatalogStore {
    pub fn for_cache_root(cache_root: &Path) -> Self {
        Self {
            path: cache_root.join("model-pricing").join(CATALOG_FILE_NAME),
        }
    }

    #[cfg(test)]
    pub fn for_test(cache_root: &Path) -> Self {
        Self::for_cache_root(cache_root)
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn load(&self) -> Result<Option<PricingCatalog>, CatalogStoreError> {
        let bytes = match fs::read(&self.path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(CatalogStoreError::Io(error)),
        };
        let catalog: PricingCatalog =
            serde_json::from_slice(&bytes).map_err(CatalogStoreError::Decode)?;
        catalog.validate_complete()?;
        Ok(Some(catalog))
    }

    pub fn save(&self, catalog: &PricingCatalog) -> Result<(), CatalogStoreError> {
        catalog.validate_complete()?;
        let encoded = serde_json::to_vec_pretty(catalog).map_err(CatalogStoreError::Encode)?;
        let Some(parent) = self.path.parent() else {
            return Err(CatalogStoreError::Io(io::Error::new(
                io::ErrorKind::InvalidInput,
                "pricing catalog cache path has no parent",
            )));
        };
        fs::create_dir_all(parent)?;
        atomic_write(&self.path, &encoded)?;
        Ok(())
    }
}

fn atomic_write(path: &Path, contents: &[u8]) -> io::Result<()> {
    atomic_write_with(path, contents, replace_file)
}

fn atomic_write_with<F>(path: &Path, contents: &[u8], replace: F) -> io::Result<()>
where
    F: FnOnce(&Path, &Path) -> io::Result<()>,
{
    let Some(parent) = path.parent() else {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "atomic cache path has no parent",
        ));
    };
    fs::create_dir_all(parent)?;
    let temporary = path.with_extension(format!("tmp-{}", Uuid::new_v4().simple()));
    let write_result = (|| {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)?;
        file.write_all(contents)?;
        file.sync_all()?;
        drop(file);
        replace(&temporary, path)?;
        sync_parent(parent)?;
        Ok(())
    })();
    if write_result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    write_result
}

#[cfg(windows)]
fn replace_file(temporary: &Path, target: &Path) -> io::Result<()> {
    if !target.exists() {
        return fs::rename(temporary, target);
    }

    use std::os::windows::ffi::OsStrExt;
    use windows::Win32::Storage::FileSystem::{REPLACE_FILE_FLAGS, ReplaceFileW};
    use windows::core::PCWSTR;

    let target_wide: Vec<u16> = target.as_os_str().encode_wide().chain(Some(0)).collect();
    let temporary_wide: Vec<u16> = temporary.as_os_str().encode_wide().chain(Some(0)).collect();
    unsafe {
        ReplaceFileW(
            PCWSTR(target_wide.as_ptr()),
            PCWSTR(temporary_wide.as_ptr()),
            PCWSTR::null(),
            REPLACE_FILE_FLAGS(0),
            None,
            None,
        )
    }
    .map_err(io::Error::other)
}

#[cfg(not(windows))]
fn replace_file(temporary: &Path, target: &Path) -> io::Result<()> {
    fs::rename(temporary, target)
}

#[cfg(unix)]
fn sync_parent(parent: &Path) -> io::Result<()> {
    File::open(parent)?.sync_all()
}

#[cfg(not(unix))]
fn sync_parent(_parent: &Path) -> io::Result<()> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pricing::catalog::{
        CatalogEntry, Currency, MoneyMicros, PriceProvenance, PricingCatalog, TokenRates,
    };
    use crate::pricing::model_alias::ModelAliasResolver;
    use chrono::{DateTime, Utc};
    use std::fs;
    use std::io;
    use tempfile::tempdir;

    fn catalog_with(model: &str) -> PricingCatalog {
        PricingCatalog::new(
            vec![CatalogEntry {
                canonical_model: model.to_string(),
                vendor: "test-vendor".to_string(),
                rates: TokenRates {
                    currency: Currency::Usd,
                    input_per_million: MoneyMicros::from_micros(1_000_000),
                    cached_input_per_million: MoneyMicros::from_micros(500_000),
                    output_per_million: MoneyMicros::from_micros(2_000_000),
                    context_tiers: Vec::new(),
                },
                source_url: "https://pricing.example.test/catalog".to_string(),
                fetched_at: DateTime::parse_from_rfc3339("2026-08-24T00:00:00Z")
                    .unwrap()
                    .with_timezone(&Utc),
                parser_revision: "fixture-v1".to_string(),
                provenance: PriceProvenance::OfficialCached,
            }],
            ModelAliasResolver::empty(),
        )
        .unwrap()
    }

    #[test]
    fn cache_write_round_trips_a_complete_catalog() {
        let dir = tempdir().unwrap();
        let store = CatalogStore::for_test(dir.path());

        store.save(&catalog_with("a")).unwrap();

        assert_eq!(store.load().unwrap().unwrap().entries.len(), 1);
    }

    #[test]
    fn incomplete_catalog_does_not_replace_the_last_complete_cache() {
        let dir = tempdir().unwrap();
        let store = CatalogStore::for_test(dir.path());
        store.save(&catalog_with("a")).unwrap();
        let previous = fs::read(store.path()).unwrap();

        assert!(matches!(
            store.save(&PricingCatalog::empty()),
            Err(CatalogStoreError::IncompleteCatalog(_))
        ));

        assert_eq!(fs::read(store.path()).unwrap(), previous);
        assert_eq!(
            store.load().unwrap().unwrap().entries[0].canonical_model,
            "a"
        );
    }

    #[test]
    fn corrupt_cache_is_rejected_without_mutating_the_file() {
        let dir = tempdir().unwrap();
        let store = CatalogStore::for_test(dir.path());
        fs::create_dir_all(store.path().parent().unwrap()).unwrap();
        fs::write(store.path(), b"not-json").unwrap();
        let previous = fs::read(store.path()).unwrap();

        assert!(matches!(store.load(), Err(CatalogStoreError::Decode(_))));
        assert_eq!(fs::read(store.path()).unwrap(), previous);
    }

    #[test]
    fn cache_rejects_non_normalized_alias_keys() {
        let dir = tempdir().unwrap();
        let store = CatalogStore::for_test(dir.path());
        let mut catalog = catalog_with("gpt-test");
        catalog.aliases = ModelAliasResolver::from_mappings([("gateway/gpt-test", "gpt-test")]);
        let mut encoded = serde_json::to_value(catalog).unwrap();
        let aliases = encoded["aliases"].as_object_mut().unwrap();
        let target = aliases.remove("gateway/gpt-test").unwrap();
        aliases.insert(" Gateway/GPT-Test ".to_string(), target);
        fs::create_dir_all(store.path().parent().unwrap()).unwrap();
        fs::write(store.path(), serde_json::to_vec_pretty(&encoded).unwrap()).unwrap();

        assert!(matches!(
            store.load(),
            Err(CatalogStoreError::IncompleteCatalog(
                CatalogValidationError::InvalidAliases
            ))
        ));
    }

    #[test]
    fn cache_rejects_non_normalized_alias_targets() {
        let dir = tempdir().unwrap();
        let store = CatalogStore::for_test(dir.path());
        let mut catalog = catalog_with("gpt-test");
        catalog.aliases = ModelAliasResolver::from_mappings([("gateway/gpt-test", "gpt-test")]);
        let mut encoded = serde_json::to_value(catalog).unwrap();
        encoded["aliases"]["gateway/gpt-test"]["canonicalModel"] =
            serde_json::Value::String(" GPT-Test ".to_string());
        fs::create_dir_all(store.path().parent().unwrap()).unwrap();
        fs::write(store.path(), serde_json::to_vec_pretty(&encoded).unwrap()).unwrap();

        assert!(matches!(
            store.load(),
            Err(CatalogStoreError::IncompleteCatalog(
                CatalogValidationError::InvalidAliases
            ))
        ));
    }

    #[test]
    fn failed_atomic_replace_preserves_the_previous_file() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("catalog.json");
        fs::write(&path, b"old").unwrap();

        let error = atomic_write_with(&path, b"new", |_temporary, _target| {
            Err(io::Error::other("simulated replace failure"))
        })
        .unwrap_err();

        assert_eq!(error.kind(), io::ErrorKind::Other);
        assert_eq!(fs::read(&path).unwrap(), b"old");
        assert_eq!(
            fs::read_dir(dir.path()).unwrap().count(),
            1,
            "temporary file must be cleaned up"
        );
    }
}
