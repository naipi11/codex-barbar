use super::catalog::{Currency, MoneyMicros, valid_public_source_url};
use super::source::PricingSourceError;
use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use thiserror::Error;
use uuid::Uuid;

const FX_FILE_NAME: &str = "usd-cny-fx-v1.json";
const MICROS: i128 = 1_000_000;
pub const FX_SOURCE_URL: &str = "https://www.chinamoney.com.cn/english/bchkpr/";
pub const FX_PARSER_REVISION: &str = "pboc-usd-cny-v1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FxRateSnapshot {
    pub base: Currency,
    pub quote: Currency,
    pub rate: MoneyMicros,
    pub observed_at: NaiveDate,
    pub source_url: String,
}

#[derive(Debug, Error)]
pub enum FxStoreError {
    #[error("unable to read or write the FX cache: {0}")]
    Io(#[from] io::Error),
    #[error("unable to encode the FX cache: {0}")]
    Encode(serde_json::Error),
    #[error("unable to decode the FX cache: {0}")]
    Decode(serde_json::Error),
    #[error("FX cache is incomplete")]
    Incomplete,
}

#[derive(Debug, Clone)]
pub struct FxStore {
    path: PathBuf,
}

impl FxStore {
    pub fn for_cache_root(cache_root: &Path) -> Self {
        Self {
            path: cache_root.join("model-pricing").join(FX_FILE_NAME),
        }
    }

    pub fn load(&self) -> Result<Option<FxRateSnapshot>, FxStoreError> {
        let bytes = match fs::read(&self.path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(FxStoreError::Io(error)),
        };
        let snapshot: FxRateSnapshot =
            serde_json::from_slice(&bytes).map_err(FxStoreError::Decode)?;
        if !is_valid_usd_cny(&snapshot) {
            return Err(FxStoreError::Incomplete);
        }
        Ok(Some(snapshot))
    }

    pub fn save(&self, snapshot: &FxRateSnapshot) -> Result<(), FxStoreError> {
        if !is_valid_usd_cny(snapshot) {
            return Err(FxStoreError::Incomplete);
        }
        let encoded = serde_json::to_vec_pretty(snapshot).map_err(FxStoreError::Encode)?;
        let Some(parent) = self.path.parent() else {
            return Err(FxStoreError::Io(io::Error::new(
                io::ErrorKind::InvalidInput,
                "FX cache path has no parent",
            )));
        };
        fs::create_dir_all(parent)?;
        atomic_write(&self.path, &encoded)?;
        Ok(())
    }
}

pub fn parse_official_usd_cny(body: &str) -> Result<FxRateSnapshot, PricingSourceError> {
    let root = serde_json::from_str::<Value>(body).map_err(|_| PricingSourceError::InvalidShape)?;
    let object = root.as_object().ok_or(PricingSourceError::InvalidShape)?;
    let base = parse_currency(object.get("base"))?;
    let quote = parse_currency(object.get("quote"))?;
    if base != Currency::Usd || quote != Currency::Cny {
        return Err(PricingSourceError::InvalidShape);
    }
    let rate = super::source::money_from_json(object.get("rate"))?
        .ok_or(PricingSourceError::InvalidShape)?;
    if rate.micros() <= 0 {
        return Err(PricingSourceError::InvalidShape);
    }
    let observed_at = object
        .get("observedAt")
        .and_then(Value::as_str)
        .and_then(|value| NaiveDate::parse_from_str(value, "%Y-%m-%d").ok())
        .ok_or(PricingSourceError::InvalidShape)?;
    let source_url = object
        .get("sourceUrl")
        .and_then(Value::as_str)
        .unwrap_or(FX_SOURCE_URL);
    if !valid_public_source_url(source_url) {
        return Err(PricingSourceError::InvalidSourceUrl);
    }
    Ok(FxRateSnapshot {
        base,
        quote,
        rate,
        observed_at,
        source_url: source_url.to_string(),
    })
}

pub fn convert_amount(
    amount: MoneyMicros,
    from: Currency,
    to: Currency,
    fx: Option<&FxRateSnapshot>,
) -> Option<MoneyMicros> {
    if from == to {
        return Some(amount);
    }
    let fx = fx.filter(|snapshot| is_valid_usd_cny(snapshot))?;
    let micros = i128::from(amount.micros());
    let rate = i128::from(fx.rate.micros());
    let converted = match (from, to) {
        (Currency::Usd, Currency::Cny) => micros.checked_mul(rate)?.checked_div(MICROS)?,
        (Currency::Cny, Currency::Usd) => micros.checked_mul(MICROS)?.checked_div(rate)?,
        _ => return None,
    };
    i64::try_from(converted).ok().map(MoneyMicros::from_micros)
}

fn parse_currency(value: Option<&Value>) -> Result<Currency, PricingSourceError> {
    match value.and_then(Value::as_str) {
        Some("USD") => Ok(Currency::Usd),
        Some("CNY") => Ok(Currency::Cny),
        _ => Err(PricingSourceError::InvalidShape),
    }
}

fn is_valid_usd_cny(snapshot: &FxRateSnapshot) -> bool {
    snapshot.base == Currency::Usd
        && snapshot.quote == Currency::Cny
        && snapshot.rate.micros() > 0
        && valid_public_source_url(&snapshot.source_url)
}

fn atomic_write(path: &Path, contents: &[u8]) -> io::Result<()> {
    let temporary = path.with_extension(format!("tmp-{}", Uuid::new_v4().simple()));
    let result = (|| {
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)?;
        file.write_all(contents)?;
        file.sync_all()?;
        drop(file);
        if path.exists() {
            fs::remove_file(path)?;
        }
        fs::rename(&temporary, path)?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

pub fn usd_micros(micros: i64) -> MoneyMicros {
    MoneyMicros::from_micros(micros)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;
    fn fixture_rate() -> FxRateSnapshot {
        parse_official_usd_cny(include_str!("fixtures/fx.json")).unwrap()
    }
    #[test]
    fn cny_conversion_requires_a_dated_usd_cny_rate() {
        let result = convert_amount(usd_micros(1_000_000), Currency::Usd, Currency::Cny, None);
        assert_eq!(result, None);
    }
    #[test]
    fn dated_usd_cny_rate_converts_with_fixed_point_math() {
        let fx = fixture_rate();
        assert_eq!(
            fx.observed_at,
            NaiveDate::from_ymd_opt(2026, 8, 24).unwrap()
        );
        let converted = convert_amount(
            usd_micros(1_000_000),
            Currency::Usd,
            Currency::Cny,
            Some(&fx),
        )
        .unwrap();
        assert_eq!(converted.micros(), 7_123_456);
    }
    #[test]
    fn same_currency_conversion_does_not_require_fx() {
        assert_eq!(
            convert_amount(usd_micros(42), Currency::Usd, Currency::Usd, None)
                .unwrap()
                .micros(),
            42
        );
    }
}
