#![allow(dead_code)]

use std::collections::HashMap;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::str::FromStr;

use anyhow::{Result, anyhow, bail};
use serde::{Deserialize, Serialize};

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
    pub last_output: f64,
    pub supply_capacity: f64,
    pub potential_demand: f64,
    pub inventory: f64,
    pub unmet_demand: f64,
    pub subsidy_rate: f64,
    pub efficiency: f64,
}

impl SectorState {
    pub fn from_definition(def: &SectorDefinition, category: IndustryCategory) -> Self {
        let base = def.base_output.max(0.1);
        Self {
            id: def.id(category),
            last_output: base,
            supply_capacity: base,
            potential_demand: base,
            inventory: 0.0,
            unmet_demand: 0.0,
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
    pub(crate) fn insert_definition(
        &mut self,
        category: IndustryCategory,
        definition: SectorDefinition,
    ) -> Result<()> {
        let id = definition.id(category);
        self.insert_sector(id, definition)
    }

    pub(crate) fn insert_sector(
        &mut self,
        id: SectorId,
        definition: SectorDefinition,
    ) -> Result<()> {
        if self.sectors.contains_key(&id) {
            return Err(anyhow!(
                "セクター定義が重複しています: {} ({})",
                id.key,
                id.category
            ));
        }
        self.sectors.insert(id, definition);
        Ok(())
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

    pub fn get_mut(&mut self, id: &SectorId) -> Option<&mut SectorDefinition> {
        self.sectors.get_mut(id)
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
    pub sales: f64,
    pub demand: f64,
    pub inventory: f64,
    pub unmet_demand: f64,
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
