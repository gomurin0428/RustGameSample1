use std::fs;
use std::path::Path;

use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};

use super::model::{IndustryCatalog, IndustryCategory, SectorDefinition};

const EMBEDDED_PRIMARY: &str = include_str!("../../../../../config/industries/primary.yaml");
const EMBEDDED_SECONDARY: &str = include_str!("../../../../../config/industries/secondary.yaml");
const EMBEDDED_TERTIARY: &str = include_str!("../../../../../config/industries/tertiary.yaml");
const EMBEDDED_ENERGY: &str = include_str!("../../../../../config/industries/energy.yaml");

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
        for entry in fs::read_dir(path).context("産業定義の読み込みに失敗しました")? {
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
            insert_category(&mut catalog, file, &entry_path)?;
        }
        Ok(catalog)
    }

    pub fn from_embedded() -> Result<Self> {
        let mut catalog = IndustryCatalog::default();
        let sources = [
            ("primary", EMBEDDED_PRIMARY),
            ("secondary", EMBEDDED_SECONDARY),
            ("tertiary", EMBEDDED_TERTIARY),
            ("energy", EMBEDDED_ENERGY),
        ];
        for (name, content) in sources {
            let file: CategoryFile = serde_yaml::from_str(content)
                .with_context(|| format!("組み込み産業定義の解析に失敗しました: {}", name))?;
            insert_category(&mut catalog, file, Path::new(name))?;
        }
        Ok(catalog)
    }
}

fn insert_category(catalog: &mut IndustryCatalog, file: CategoryFile, path: &Path) -> Result<()> {
    for sector in file.sectors {
        catalog
            .insert_definition(file.category, sector)
            .with_context(|| format!("セクター挿入に失敗しました: {}", path.display()))?;
    }
    Ok(())
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
            super::super::model::DependencyKind::Cost
        ));
    }

    #[test]
    fn catalog_rejects_duplicates() {
        let mut catalog = IndustryCatalog::default();
        catalog
            .insert_definition(
                IndustryCategory::Primary,
                SectorDefinition {
                    key: "wheat".into(),
                    name: "小麦".into(),
                    description: None,
                    base_output: 100.0,
                    base_cost: 40.0,
                    price_sensitivity: 0.5,
                    employment: 100.0,
                    dependencies: Vec::new(),
                },
            )
            .expect("初回登録");
        let result = catalog.insert_definition(
            IndustryCategory::Primary,
            SectorDefinition {
                key: "wheat".into(),
                name: "小麦".into(),
                description: None,
                base_output: 120.0,
                base_cost: 45.0,
                price_sensitivity: 0.6,
                employment: 110.0,
                dependencies: Vec::new(),
            },
        );
        assert!(result.is_err());
    }
}
