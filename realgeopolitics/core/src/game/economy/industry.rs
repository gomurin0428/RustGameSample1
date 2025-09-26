#![allow(dead_code)]

use std::collections::HashMap;
use std::fmt;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::Path;
use std::str::FromStr;

use anyhow::{Context, Result, anyhow, bail, ensure};
use serde::{Deserialize, Serialize};

const EMBEDDED_PRIMARY: &str = include_str!("../../../../config/industries/primary.yaml");
const EMBEDDED_SECONDARY: &str = include_str!("../../../../config/industries/secondary.yaml");
const EMBEDDED_TERTIARY: &str = include_str!("../../../../config/industries/tertiary.yaml");
const EMBEDDED_ENERGY: &str = include_str!("../../../../config/industries/energy.yaml");

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash, PartialOrd, Ord)]
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

    pub fn as_str(self) -> &'static str {
        match self {
            IndustryCategory::Primary => "primary",
            IndustryCategory::Secondary => "secondary",
            IndustryCategory::Tertiary => "tertiary",
            IndustryCategory::Energy => "energy",
        }
    }
}

impl fmt::Display for IndustryCategory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl FromStr for IndustryCategory {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self> {
        let normalized = s.trim().to_ascii_lowercase();
        match normalized.as_str() {
            "primary" | "一次" | "1" => Ok(IndustryCategory::Primary),
            "secondary" | "二次" | "2" => Ok(IndustryCategory::Secondary),
            "tertiary" | "三次" | "3" => Ok(IndustryCategory::Tertiary),
            "energy" | "エネルギー" | "4" => Ok(IndustryCategory::Energy),
            other => bail!("未知の産業カテゴリです: {}", other),
        }
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
            catalog.merge_category(file, Path::new(name))?;
        }
        Ok(catalog)
    }

    pub fn sectors(&self) -> impl Iterator<Item = (&SectorId, &SectorDefinition)> {
        self.sectors.iter()
    }

    pub fn sectors_by_category(
        &self,
        category: IndustryCategory,
    ) -> impl Iterator<Item = (&SectorId, &SectorDefinition)> {
        self.sectors
            .iter()
            .filter(move |(id, _)| id.category == category)
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

#[derive(Debug, Clone, Copy, Default)]
pub struct SectorModifier {
    pub subsidy_bonus: f64,
    pub efficiency_bonus: f64,
    pub remaining_minutes: f64,
}

impl SectorModifier {
    pub fn decay(&mut self, minutes: f64) {
        if self.remaining_minutes <= 0.0 {
            self.subsidy_bonus = 0.0;
            self.efficiency_bonus = 0.0;
            return;
        }
        let decay = minutes.max(0.0);
        if decay <= 0.0 {
            return;
        }
        self.remaining_minutes = (self.remaining_minutes - decay).max(0.0);
        if self.remaining_minutes == 0.0 {
            self.subsidy_bonus = 0.0;
            self.efficiency_bonus = 0.0;
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct SectorMetrics {
    pub output: f64,
    pub revenue: f64,
    pub cost: f64,
}

#[derive(Debug, Clone)]
pub struct SectorOverview {
    pub id: SectorId,
    pub name: String,
    pub category: IndustryCategory,
    pub subsidy_percent: f64,
    pub last_output: f64,
    pub last_revenue: f64,
    pub last_cost: f64,
}

#[derive(Debug, Default)]
pub struct IndustryTickOutcome {
    pub total_revenue: f64,
    pub total_cost: f64,
    pub total_gdp: f64,
    pub sector_metrics: HashMap<SectorId, SectorMetrics>,
    pub reports: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct IndustryRuntime {
    catalog: IndustryCatalog,
    states: HashMap<SectorId, SectorState>,
    modifiers: HashMap<SectorId, SectorModifier>,
    last_metrics: HashMap<SectorId, SectorMetrics>,
    energy_baseline_output: f64,
    energy_cost_index: f64,
}

impl IndustryRuntime {
    pub fn from_catalog(catalog: IndustryCatalog) -> Self {
        let mut states = HashMap::new();
        let mut energy_baseline = 0.0;
        for (id, def) in catalog.sectors() {
            if id.category == IndustryCategory::Energy {
                energy_baseline += def.base_output;
            }
            states.insert(id.clone(), SectorState::from_definition(def, id.category));
        }
        Self {
            catalog,
            states,
            modifiers: HashMap::new(),
            last_metrics: HashMap::new(),
            energy_baseline_output: energy_baseline.max(1.0),
            energy_cost_index: 1.0,
        }
    }

    pub fn simulate_tick(&mut self, minutes: f64, scale: f64) -> IndustryTickOutcome {
        let mut outcome = IndustryTickOutcome::default();
        const ORDER: [IndustryCategory; 4] = [
            IndustryCategory::Energy,
            IndustryCategory::Primary,
            IndustryCategory::Secondary,
            IndustryCategory::Tertiary,
        ];
        let mut energy_output_total = 0.0;
        for category in ORDER {
            let sector_ids: Vec<SectorId> = self
                .catalog
                .sectors_by_category(category)
                .map(|(id, _)| id.clone())
                .collect();
            for sector_id in sector_ids {
                let def = match self.catalog.get(&sector_id) {
                    Some(def) => def,
                    None => continue,
                };
                let (input_factor, cost_factor_raw, demand_signal) =
                    Self::compute_dependency_effects(category, def, &outcome.sector_metrics);
                let state_entry = self
                    .states
                    .entry(sector_id.clone())
                    .or_insert_with(|| SectorState::from_definition(def, category));
                let modifier = self
                    .modifiers
                    .entry(sector_id.clone())
                    .or_insert_with(SectorModifier::default);

                let subsidy = modifier.subsidy_bonus.clamp(0.0, 0.9);
                let efficiency =
                    (state_entry.efficiency * (1.0 + modifier.efficiency_bonus)).max(0.1);

                let input_factor = input_factor.max(0.0);
                let mut cost_factor = cost_factor_raw;
                if category != IndustryCategory::Energy {
                    cost_factor *= self.energy_cost_index;
                }
                let output = def.base_output * efficiency * input_factor * scale;
                let mut cost = def.base_cost * cost_factor * scale * (1.0 - subsidy);
                if cost.is_nan() || cost.is_infinite() || cost < 0.0 {
                    cost = 0.0;
                }
                let price_multiplier = sigmoid_price(demand_signal, def.price_sensitivity);
                let price = (def.base_cost * price_multiplier).max(0.1);
                let revenue = output * price;
                let gdp_contrib = (revenue - cost).max(0.0);

                state_entry.output = output;
                state_entry.subsidy_rate = subsidy;
                state_entry.efficiency = (state_entry.efficiency * 0.95) + (efficiency * 0.05);
                modifier.decay(minutes);

                if category == IndustryCategory::Energy {
                    energy_output_total += output;
                }

                let metrics = SectorMetrics {
                    output,
                    revenue,
                    cost,
                };
                outcome.total_revenue += revenue;
                outcome.total_cost += cost;
                outcome.total_gdp += gdp_contrib;
                outcome
                    .sector_metrics
                    .insert(sector_id.clone(), metrics.clone());

                if output > f64::EPSILON {
                    outcome.reports.push(format!(
                        "{}: 生産量 {:.1} / 収益 {:.1}",
                        def.name, output, revenue
                    ));
                }
            }

            if category == IndustryCategory::Energy {
                self.energy_cost_index = if energy_output_total <= f64::EPSILON {
                    1.5
                } else {
                    (self.energy_baseline_output / energy_output_total).clamp(0.5, 1.6)
                };
            }
        }
        self.last_metrics = outcome.sector_metrics.clone();
        outcome
    }

    pub fn resolve_sector_token(&self, token: &str) -> Result<SectorId> {
        let raw = token.trim();
        ensure!(!raw.is_empty(), "セクターを指定してください。");
        let mut splits = raw.split(|c| c == ':' || c == '/');
        let first = splits.next().expect("split は少なくとも1要素");
        if let Some(second) = splits.next() {
            let category = IndustryCategory::from_str(first)?;
            let key = second.trim();
            ensure!(!key.is_empty(), "セクターキーが空です。");
            let id = SectorId::new(category, key);
            ensure!(
                self.catalog.get(&id).is_some(),
                "指定されたセクターは存在しません: {}",
                raw
            );
            return Ok(id);
        }

        let mut matches = self
            .catalog
            .sectors()
            .filter(|(id, _)| id.key.eq_ignore_ascii_case(raw))
            .map(|(id, _)| id.clone())
            .collect::<Vec<_>>();
        matches.sort_by(|a, b| a.category.cmp(&b.category));
        match matches.len() {
            0 => bail!("セクターが見つかりません: {}", raw),
            1 => Ok(matches.remove(0)),
            _ => bail!(
                "セクター名が複数カテゴリに存在します: {} (category:key 形式で指定してください)",
                raw
            ),
        }
    }

    pub fn apply_subsidy(&mut self, id: &SectorId, percent: f64) -> Result<SectorOverview> {
        ensure!(self.catalog.get(id).is_some(), "セクターが存在しません。");
        ensure!(
            percent.is_finite(),
            "補助率は有限の数値で指定してください。"
        );
        ensure!(percent >= 0.0, "補助率は0%以上で指定してください。");
        let ratio = (percent / 100.0).clamp(0.0, 0.9);
        let modifier = self.modifiers.entry(id.clone()).or_default();
        modifier.subsidy_bonus = ratio;
        modifier.remaining_minutes = f64::MAX;
        if let Some(state) = self.states.get_mut(id) {
            state.subsidy_rate = ratio;
        }
        self.overview_for(id)
    }

    pub fn overview(&self) -> Vec<SectorOverview> {
        let mut entries = Vec::new();
        for (id, def) in self.catalog.sectors() {
            let state = self.states.get(id);
            let metrics = self.last_metrics.get(id);
            entries.push(SectorOverview {
                id: id.clone(),
                name: def.name.clone(),
                category: id.category,
                subsidy_percent: state.map(|s| s.subsidy_rate * 100.0).unwrap_or(0.0),
                last_output: metrics.map(|m| m.output).unwrap_or(0.0),
                last_revenue: metrics.map(|m| m.revenue).unwrap_or(0.0),
                last_cost: metrics.map(|m| m.cost).unwrap_or(0.0),
            });
        }
        entries.sort_by(|a, b| {
            a.category
                .cmp(&b.category)
                .then_with(|| a.name.cmp(&b.name))
        });
        entries
    }

    pub fn overview_for(&self, id: &SectorId) -> Result<SectorOverview> {
        let def = self
            .catalog
            .get(id)
            .ok_or_else(|| anyhow!("セクターが存在しません: {}: {}", id.category, id.key))?;
        let state = self.states.get(id);
        let metrics = self.last_metrics.get(id);
        Ok(SectorOverview {
            id: id.clone(),
            name: def.name.clone(),
            category: id.category,
            subsidy_percent: state.map(|s| s.subsidy_rate * 100.0).unwrap_or(0.0),
            last_output: metrics.map(|m| m.output).unwrap_or(0.0),
            last_revenue: metrics.map(|m| m.revenue).unwrap_or(0.0),
            last_cost: metrics.map(|m| m.cost).unwrap_or(0.0),
        })
    }

    fn compute_dependency_effects(
        category: IndustryCategory,
        def: &SectorDefinition,
        metrics: &HashMap<SectorId, SectorMetrics>,
    ) -> (f64, f64, f64) {
        let mut input_factor = 1.0;
        let mut cost_factor = 1.0;
        let mut demand_signal = 0.0;
        for dep in &def.dependencies {
            let dep_id = dep.resolve_sector(category);
            let supply_ratio = metrics
                .get(&dep_id)
                .map(|m| {
                    let requirement = dep.requirement.max(0.01);
                    (m.output / (def.base_output * requirement)).clamp(0.0, 2.0)
                })
                .unwrap_or(0.0);
            match dep.dependency {
                DependencyKind::Input => {
                    let shortage = (0.8 - supply_ratio).max(0.0);
                    let surplus = (supply_ratio - 1.2).max(0.0);
                    input_factor *= (1.0 - shortage).clamp(0.0, 1.0);
                    if surplus > 0.0 {
                        input_factor *= 1.0 + (surplus.min(0.5) * 0.05);
                    }
                }
                DependencyKind::Cost => {
                    let adjustment = 1.0 - dep.elasticity * (supply_ratio - 1.0);
                    cost_factor *= adjustment.clamp(0.5, 1.5);
                }
                DependencyKind::Demand => {
                    demand_signal += dep.elasticity * (supply_ratio - 1.0);
                }
            }
        }
        (input_factor.max(0.0), cost_factor.max(0.1), demand_signal)
    }

    pub fn metrics(&self) -> &HashMap<SectorId, SectorMetrics> {
        &self.last_metrics
    }

    pub fn catalog(&self) -> &IndustryCatalog {
        &self.catalog
    }

    pub fn energy_cost_index(&self) -> f64 {
        self.energy_cost_index
    }

    #[cfg(test)]
    pub fn set_modifier_for_test(
        &mut self,
        id: &SectorId,
        subsidy_bonus: f64,
        efficiency_bonus: f64,
        duration_minutes: f64,
    ) {
        let modifier = self.modifiers.entry(id.clone()).or_default();
        modifier.subsidy_bonus = subsidy_bonus;
        modifier.efficiency_bonus = efficiency_bonus;
        modifier.remaining_minutes = duration_minutes.max(0.0);
    }
}

fn sigmoid_price(signal: f64, sensitivity: f64) -> f64 {
    let logistic = 1.0 / (1.0 + (-3.0 * signal).exp());
    let centered = (logistic - 0.5) * 2.0;
    (1.0 + sensitivity * centered).clamp(0.2, 2.5)
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

    #[test]
    fn energy_supply_reduces_cost_index() {
        let catalog = IndustryCatalog::from_embedded().expect("catalog");
        let mut runtime = IndustryRuntime::from_catalog(catalog);
        let outcome = runtime.simulate_tick(60.0, 1.0);
        assert!(outcome.total_revenue > 0.0);
        assert!(runtime.energy_cost_index() >= 0.5);
    }

    #[test]
    fn dependency_shortage_reduces_output() {
        let mut catalog = IndustryCatalog::default();
        let energy_id = SectorId::new(IndustryCategory::Energy, "electricity");
        catalog.sectors.insert(
            energy_id.clone(),
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
        );
        let auto_id = SectorId::new(IndustryCategory::Secondary, "automotive");
        catalog.sectors.insert(
            auto_id.clone(),
            SectorDefinition {
                key: "automotive".into(),
                name: "自動車".into(),
                description: None,
                base_output: 150.0,
                base_cost: 120.0,
                price_sensitivity: 0.4,
                employment: 110.0,
                dependencies: vec![SectorDependency {
                    sector: "electricity".into(),
                    category: Some(IndustryCategory::Energy),
                    requirement: 1.5,
                    elasticity: 0.0,
                    dependency: DependencyKind::Input,
                }],
            },
        );

        let mut baseline_runtime = IndustryRuntime::from_catalog(catalog.clone());
        let baseline_output = baseline_runtime
            .simulate_tick(60.0, 1.0)
            .sector_metrics
            .get(&auto_id)
            .expect("baseline automotive metrics")
            .output;

        let mut shortage_runtime = IndustryRuntime::from_catalog(catalog);
        shortage_runtime.set_modifier_for_test(&energy_id, 0.0, -0.9, 120.0);
        let shortage_output = shortage_runtime
            .simulate_tick(60.0, 1.0)
            .sector_metrics
            .get(&auto_id)
            .expect("shortage automotive metrics")
            .output;
        assert!(
            shortage_output < baseline_output * 0.35,
            "expected shortage output ({shortage_output}) to fall below 35% of baseline ({baseline_output})"
        );
    }

    #[test]
    fn demand_signal_adjusts_price() {
        let mut catalog = IndustryCatalog::from_embedded().expect("catalog");
        if let Some(def) = catalog
            .sectors
            .get_mut(&SectorId::new(IndustryCategory::Tertiary, "finance"))
        {
            def.dependencies.push(SectorDependency {
                sector: "automotive".into(),
                category: Some(IndustryCategory::Secondary),
                requirement: 0.5,
                elasticity: 0.6,
                dependency: DependencyKind::Demand,
            });
        }
        let mut runtime = IndustryRuntime::from_catalog(catalog);
        let outcome = runtime.simulate_tick(60.0, 1.0);
        let finance_id = SectorId::new(IndustryCategory::Tertiary, "finance");
        let metrics = outcome
            .sector_metrics
            .get(&finance_id)
            .expect("finance metrics");
        assert!(metrics.revenue >= metrics.cost);
    }
}
