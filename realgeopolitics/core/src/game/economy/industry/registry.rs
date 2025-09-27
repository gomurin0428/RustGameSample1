use std::collections::HashMap;
use std::str::FromStr;

use anyhow::{Result, bail, ensure};

use super::model::{IndustryCatalog, IndustryCategory, SectorId};

#[derive(Debug, Clone)]
pub struct SectorRegistry {
    by_key: HashMap<String, Vec<SectorId>>,
}

impl SectorRegistry {
    pub fn from_catalog(catalog: &IndustryCatalog) -> Self {
        let mut by_key: HashMap<String, Vec<SectorId>> = HashMap::new();
        for (id, _) in catalog.sectors() {
            by_key
                .entry(id.key.to_ascii_lowercase())
                .or_default()
                .push(id.clone());
        }
        for entries in by_key.values_mut() {
            entries.sort_by(|a, b| a.category.cmp(&b.category).then_with(|| a.key.cmp(&b.key)));
        }
        Self { by_key }
    }

    pub fn resolve(&self, token: &str) -> Result<SectorId> {
        let raw = token.trim();
        ensure!(!raw.is_empty(), "セクターを指定してください。");
        let mut splits = raw.split(|c| c == ':' || c == '/');
        let first = splits.next().expect("split は少なくとも1要素");
        if let Some(second) = splits.next() {
            let category = IndustryCategory::from_str(first)?;
            let key = second.trim();
            ensure!(!key.is_empty(), "セクターキーが空です。");
            if let Some(entries) = self.by_key.get(&key.to_ascii_lowercase()) {
                if let Some(found) = entries
                    .iter()
                    .find(|id| id.category == category && id.key.eq_ignore_ascii_case(key))
                {
                    return Ok(found.clone());
                }
            }
            bail!("指定されたセクターは存在しません: {}", raw);
        }

        let lowered = raw.to_ascii_lowercase();
        match self.by_key.get(&lowered) {
            None => bail!("セクターが見つかりません: {}", raw),
            Some(entries) if entries.len() == 1 => Ok(entries[0].clone()),
            Some(_) => bail!(
                "セクター名が複数カテゴリに存在します: {} (category:key 形式で指定してください)",
                raw
            ),
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = &SectorId> {
        self.by_key.values().flat_map(|ids| ids.iter())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::economy::{IndustryCatalog, SectorDefinition};

    fn catalog_with_samples() -> IndustryCatalog {
        let mut catalog = IndustryCatalog::default();
        catalog
            .insert_definition(
                IndustryCategory::Primary,
                SectorDefinition {
                    key: "grain".into(),
                    name: "穀物".into(),
                    description: None,
                    base_output: 100.0,
                    base_cost: 50.0,
                    price_sensitivity: 0.5,
                    employment: 80.0,
                    dependencies: Vec::new(),
                },
            )
            .expect("insert grain");
        catalog
            .insert_definition(
                IndustryCategory::Secondary,
                SectorDefinition {
                    key: "automotive".into(),
                    name: "自動車".into(),
                    description: None,
                    base_output: 160.0,
                    base_cost: 120.0,
                    price_sensitivity: 0.4,
                    employment: 110.0,
                    dependencies: Vec::new(),
                },
            )
            .expect("insert automotive");
        catalog
            .insert_definition(
                IndustryCategory::Tertiary,
                SectorDefinition {
                    key: "automotive".into(),
                    name: "自動車サービス".into(),
                    description: None,
                    base_output: 90.0,
                    base_cost: 70.0,
                    price_sensitivity: 0.3,
                    employment: 60.0,
                    dependencies: Vec::new(),
                },
            )
            .expect("insert automotive tertiary");
        catalog
    }

    #[test]
    fn resolve_category_key_pair() {
        let catalog = catalog_with_samples();
        let registry = SectorRegistry::from_catalog(&catalog);
        let sector = registry.resolve("primary:grain").expect("resolve grain");
        assert_eq!(sector.category, IndustryCategory::Primary);
        assert_eq!(sector.key, "grain");
    }

    #[test]
    fn resolve_unique_key_without_category() {
        let mut catalog = catalog_with_samples();
        catalog
            .insert_definition(
                IndustryCategory::Energy,
                SectorDefinition {
                    key: "electricity".into(),
                    name: "電力".into(),
                    description: None,
                    base_output: 200.0,
                    base_cost: 80.0,
                    price_sensitivity: 0.3,
                    employment: 90.0,
                    dependencies: Vec::new(),
                },
            )
            .expect("insert electricity");
        let registry = SectorRegistry::from_catalog(&catalog);
        let sector = registry
            .resolve("electricity")
            .expect("resolve electricity");
        assert_eq!(sector.category, IndustryCategory::Energy);
    }

    #[test]
    fn resolve_requires_category_when_duplicate_keys_exist() {
        let catalog = catalog_with_samples();
        let registry = SectorRegistry::from_catalog(&catalog);
        let err = registry.resolve("automotive").expect_err("ambiguous");
        assert!(
            err.to_string()
                .contains("セクター名が複数カテゴリに存在します")
        );
    }
}
