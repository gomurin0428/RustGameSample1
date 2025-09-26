#![allow(dead_code)]

use std::collections::HashMap;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::Path;

use anyhow::{Context, Result, anyhow};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum IndustryCategory {
    Primary,
    Secondary,
    Tertiary,
    Energy,
}

impl IndustryCategory {
    pub fn iter() -> impl Iterator<Item = IndustryCategory> {
        [
            IndustryCategory::Primary,
            IndustryCategory::Secondary,
            IndustryCategory::Tertiary,
            IndustryCategory::Energy,
        ]
        .into_iter()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SectorId {
    pub category: IndustryCategory,
    pub key: String,
}

impl SectorId {
    pub fn new<C: Into<String>>(category: IndustryCategory, key: C) -> Self {
        Self {
            category,
            key: key.into(),
        }
    }
}

impl Hash for SectorId {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.category.hash(state);
        self.key.hash(state);
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SectorDependency {
    pub sector: String,
    #[serde(default)]
    pub category: Option<IndustryCategory>,
    #[serde(default = "SectorDependency::default_requirement")]
    pub requirement: f64,
    #[serde(default = "SectorDependency::default_elasticity")]
    pub elasticity: f64,
    #[serde(default)]
    pub dependency: DependencyKind,
}

impl SectorDependency {
    const fn default_requirement() -> f64 {
        1.0
    }

    const fn default_elasticity() -> f64 {
        0.0
    }

    pub fn resolve_sector(&self, fallback_category: IndustryCategory) -> SectorId {
        SectorId::new(self.category.unwrap_or(fallback_category), &self.sector)
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum DependencyKind {
    Input,
    Cost,
    Demand,
}

impl Default for DependencyKind {
    fn default() -> Self {
        DependencyKind::Input
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SectorDefinition {
    pub key: String,
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default = "SectorDefinition::default_output")]
    pub base_output: f64,
    #[serde(default = "SectorDefinition::default_cost")]
    pub base_cost: f64,
    #[serde(default = "SectorDefinition::default_price_sensitivity")]
    pub price_sensitivity: f64,
    #[serde(default = "SectorDefinition::default_employment")]
    pub employment: f64,
    #[serde(default)]
    pub dependencies: Vec<SectorDependency>,
}

impl SectorDefinition {
    const fn default_output() -> f64 {
        100.0
    }

    const fn default_cost() -> f64 {
        50.0
    }

    const fn default_price_sensitivity() -> f64 {
        0.5
    }

    const fn default_employment() -> f64 {
        100.0
    }

    pub fn id(&self, category: IndustryCategory) -> SectorId {
        SectorId::new(category, &self.key)
    }
}

#[derive(Debug, Clone)]
pub struct SectorState {
    pub id: SectorId,
    pub output: f64,
    pub capacity: f64,
    pub subsidy_rate: f64,
    pub efficiency: f64,
}

impl SectorState {
    pub fn from_definition(def: &SectorDefinition, category: IndustryCategory) -> Self {
        Self {
            id: def.id(category),
            output: def.base_output,
            capacity: 1.0,
            subsidy_rate: 0.0,
            efficiency: 1.0,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct IndustryCatalog {
    sectors: HashMap<SectorId, SectorDefinition>,
}

impl IndustryCatalog {
    pub fn load_from_dir<P: AsRef<Path>>(dir: P) -> Result<Self> {
        let path = dir.as_ref();
        if !path.exists() {
            return Err(anyhow!(
                "産業定義ディレクトリが存在しません: {}",
                path.display()
            ));
        }

        let mut catalog = IndustryCatalog::default();
        for entry in fs::read_dir(path).context("産業定義の読み込みに失敗しました")?
        {
            let entry = entry?;
            let entry_path = entry.path();
            if !is_yaml_file(&entry_path) {
                continue;
            }
            let content = fs::read_to_string(&entry_path).with_context(|| {
                format!("ファイルの読み込みに失敗しました: {}", entry_path.display())
            })?;
            let file: CategoryFile = serde_yaml::from_str(&content).with_context(|| {
                format!(
                    "産業定義 YAML の解析に失敗しました: {}",
                    entry_path.display()
                )
            })?;
            catalog.merge_category(file, &entry_path)?;
        }
        Ok(catalog)
    }

    pub fn sectors(&self) -> impl Iterator<Item = (&SectorId, &SectorDefinition)> {
        self.sectors.iter()
    }

    pub fn get(&self, id: &SectorId) -> Option<&SectorDefinition> {
        self.sectors.get(id)
    }

    fn merge_category(&mut self, file: CategoryFile, path: &Path) -> Result<()> {
        for sector in file.sectors {
            let id = sector.id(file.category);
            if self.sectors.contains_key(&id) {
                return Err(anyhow!(
                    "セクター定義が重複しています: {} ({})",
                    id.key,
                    path.display()
                ));
            }
            self.sectors.insert(id, sector);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CategoryFile {
    pub category: IndustryCategory,
    #[serde(default)]
    pub sectors: Vec<SectorDefinition>,
}

fn is_yaml_file(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|s| s.to_str()),
        Some("yaml" | "yml")
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn category_file_deserialises() {
        let yaml = r#"
category: primary
sectors:
  - key: wheat
    name: 小麦
    base_output: 120
    base_cost: 35
    dependencies:
      - sector: electricity
        category: energy
        requirement: 0.25
        dependency: cost
  - key: vegetables
    name: 野菜
"#;
        let file: CategoryFile = serde_yaml::from_str(yaml).expect("YAML を解析");
        assert_eq!(file.category, IndustryCategory::Primary);
        assert_eq!(file.sectors.len(), 2);
        let wheat = &file.sectors[0];
        assert_eq!(wheat.key, "wheat");
        assert_eq!(wheat.dependencies.len(), 1);
        assert!(matches!(
            wheat.dependencies[0].dependency,
            DependencyKind::Cost
        ));
    }

    #[test]
    fn catalog_rejects_duplicates() {
        let mut catalog = IndustryCatalog::default();
        let category = CategoryFile {
            category: IndustryCategory::Primary,
            sectors: vec![SectorDefinition {
                key: "wheat".into(),
                name: "小麦".into(),
                description: None,
                base_output: 100.0,
                base_cost: 40.0,
                price_sensitivity: 0.5,
                employment: 100.0,
                dependencies: Vec::new(),
            }],
        };
        catalog
            .merge_category(category, Path::new("inline"))
            .expect("初回登録");
        let duplicate = CategoryFile {
            category: IndustryCategory::Primary,
            sectors: vec![SectorDefinition {
                key: "wheat".into(),
                name: "小麦".into(),
                description: None,
                base_output: 120.0,
                base_cost: 45.0,
                price_sensitivity: 0.6,
                employment: 110.0,
                dependencies: Vec::new(),
            }],
        };
        let result = catalog.merge_category(duplicate, Path::new("inline"));
        assert!(result.is_err());
    }
}
