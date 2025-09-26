use std::collections::HashMap;
use std::str::FromStr;

use anyhow::{Result, bail, ensure};

use super::model::{
    DependencyKind, IndustryCatalog, IndustryCategory, SectorDefinition, SectorId, SectorMetrics,
    SectorModifier, SectorState,
};

#[derive(Debug, Clone, Copy)]
pub(crate) struct DependencyImpact {
    pub input_availability: f64,
    pub cost_multiplier: f64,
    pub demand_multiplier: f64,
}

impl Default for DependencyImpact {
    fn default() -> Self {
        Self {
            input_availability: 1.0,
            cost_multiplier: 1.0,
            demand_multiplier: 1.0,
        }
    }
}

pub(crate) fn resolve_sector_token(catalog: &IndustryCatalog, token: &str) -> Result<SectorId> {
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
            catalog.get(&id).is_some(),
            "指定されたセクターは存在しません: {}",
            raw
        );
        return Ok(id);
    }

    let mut matches = catalog
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

pub(crate) fn apply_subsidy(
    catalog: &IndustryCatalog,
    modifiers: &mut HashMap<SectorId, SectorModifier>,
    states: &mut HashMap<SectorId, SectorState>,
    id: &SectorId,
    percent: f64,
) -> Result<()> {
    ensure!(catalog.get(id).is_some(), "セクターが存在しません。");
    ensure!(
        percent.is_finite(),
        "補助率は有限の数値で指定してください。"
    );
    ensure!(percent >= 0.0, "補助率は0%以上で指定してください。");
    let ratio = (percent / 100.0).clamp(0.0, 0.9);
    let modifier = modifiers.entry(id.clone()).or_default();
    modifier.subsidy_bonus = ratio;
    modifier.remaining_minutes = f64::MAX;
    if let Some(state) = states.get_mut(id) {
        state.subsidy_rate = ratio;
    }
    Ok(())
}

pub(crate) fn evaluate_dependency_impacts(
    category: IndustryCategory,
    def: &SectorDefinition,
    metrics: &HashMap<SectorId, SectorMetrics>,
    states: &HashMap<SectorId, SectorState>,
) -> DependencyImpact {
    let mut impact = DependencyImpact::default();
    if def.dependencies.is_empty() {
        return impact;
    }

    for dep in &def.dependencies {
        let dep_id = dep.resolve_sector(category);
        let requirement = (def.base_output * dep.requirement.max(0.01)).max(0.1);
        let dep_metrics = metrics.get(&dep_id);
        let dep_state = states.get(&dep_id);

        match dep.dependency {
            DependencyKind::Input => {
                let available_supply = dep_metrics
                    .map(|m| m.output + m.inventory)
                    .or_else(|| dep_state.map(|s| s.last_output + s.inventory))
                    .unwrap_or(0.0);
                let ratio = if requirement <= 0.0 {
                    1.0
                } else {
                    (available_supply / requirement).clamp(0.0, 2.0)
                };
                impact.input_availability = (impact.input_availability * ratio).clamp(0.0, 1.5);
            }
            DependencyKind::Cost => {
                let observed_supply = dep_metrics
                    .map(|m| m.output)
                    .or_else(|| dep_state.map(|s| s.last_output))
                    .unwrap_or(requirement);
                let supply_ratio = if requirement <= 0.0 {
                    1.0
                } else {
                    (observed_supply / requirement).clamp(0.1, 3.0)
                };
                let elasticity = dep.elasticity.clamp(-2.0, 2.0);
                let adjustment = 1.0 - elasticity * (1.0 - supply_ratio);
                impact.cost_multiplier = (impact.cost_multiplier * adjustment).clamp(0.4, 2.0);
            }
            DependencyKind::Demand => {
                let observed_demand = dep_metrics
                    .map(|m| m.output + m.unmet_demand)
                    .or_else(|| dep_state.map(|s| s.last_output + s.unmet_demand))
                    .unwrap_or(requirement);
                let baseline = dep_state
                    .map(|s| s.potential_demand.max(0.1))
                    .unwrap_or(requirement.max(0.1));
                let ratio = if baseline <= 0.0 {
                    1.0
                } else {
                    (observed_demand / baseline).clamp(0.0, 3.0)
                };
                let elasticity = dep.elasticity.clamp(-2.5, 2.5);
                impact.demand_multiplier =
                    (impact.demand_multiplier * (1.0 + elasticity * (ratio - 1.0))).clamp(0.2, 3.0);
            }
        }
    }

    if impact.input_availability < 0.0 || !impact.input_availability.is_finite() {
        impact.input_availability = 0.0;
    }

    impact
}

pub(crate) fn price_from_gap(gap_ratio: f64, sensitivity: f64) -> f64 {
    if !gap_ratio.is_finite() {
        return 1.0;
    }
    let clamp_ratio = gap_ratio.clamp(-1.5, 1.5);
    let logistic = 1.0 / (1.0 + (-4.0 * clamp_ratio).exp());
    let centered = (logistic - 0.5) * 2.0;
    let effective_sensitivity = sensitivity.clamp(0.1, 2.5);
    (1.0 + centered * effective_sensitivity).clamp(0.3, 2.8)
}

pub(crate) fn update_energy_cost_index(baseline_output: f64, energy_output_total: f64) -> f64 {
    if energy_output_total <= f64::EPSILON {
        1.5
    } else {
        (baseline_output / energy_output_total).clamp(0.5, 1.6)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    use crate::game::economy::industry::model::{
        DependencyKind, IndustryCatalog, SectorDefinition, SectorDependency, SectorId,
        SectorMetrics, SectorState,
    };

    #[test]
    fn price_from_gap_handles_non_finite_signal() {
        assert_eq!(price_from_gap(f64::NAN, 0.8), 1.0);
        let infinite_price = price_from_gap(f64::INFINITY, 0.8);
        assert!(infinite_price.is_finite());
        assert!(infinite_price >= 1.0 && infinite_price <= 2.8);
        let negative_price = price_from_gap(f64::NEG_INFINITY, 0.8);
        assert!(negative_price.is_finite());
        assert!(negative_price >= 0.3 && negative_price <= 1.0);
    }

    #[test]
    fn evaluate_dependency_impacts_uses_state_fallbacks() {
        let mut catalog = IndustryCatalog::default();
        let energy_def = SectorDefinition {
            key: "energy".into(),
            name: "エネルギー".into(),
            description: None,
            base_output: 100.0,
            base_cost: 80.0,
            price_sensitivity: 0.4,
            employment: 50.0,
            dependencies: Vec::new(),
        };
        catalog
            .insert_definition(IndustryCategory::Energy, energy_def.clone())
            .expect("insert energy");

        let manufacturing_def = SectorDefinition {
            key: "manufacturing".into(),
            name: "製造".into(),
            description: None,
            base_output: 120.0,
            base_cost: 90.0,
            price_sensitivity: 0.5,
            employment: 80.0,
            dependencies: vec![SectorDependency {
                sector: "energy".into(),
                category: Some(IndustryCategory::Energy),
                requirement: 1.2,
                elasticity: 0.6,
                dependency: DependencyKind::Input,
            }],
        };

        let energy_id = SectorId::new(IndustryCategory::Energy, "energy");
        let mut states = HashMap::new();
        let mut energy_state = SectorState::from_definition(&energy_def, IndustryCategory::Energy);
        energy_state.last_output = 50.0;
        energy_state.inventory = 80.0;
        states.insert(energy_id.clone(), energy_state);

        let metrics: HashMap<SectorId, SectorMetrics> = HashMap::new();
        let impact = evaluate_dependency_impacts(
            IndustryCategory::Secondary,
            &manufacturing_def,
            &metrics,
            &states,
        );
        assert!(impact.input_availability > 0.9);
        assert!(impact.cost_multiplier <= 1.0);
    }

    #[test]
    fn evaluate_dependency_impacts_combines_demand_signals() {
        let mut catalog = IndustryCatalog::default();
        let service_def = SectorDefinition {
            key: "services".into(),
            name: "サービス".into(),
            description: None,
            base_output: 150.0,
            base_cost: 60.0,
            price_sensitivity: 0.45,
            employment: 95.0,
            dependencies: Vec::new(),
        };
        catalog
            .insert_definition(IndustryCategory::Tertiary, service_def.clone())
            .expect("insert services");

        let finance_def = SectorDefinition {
            key: "finance".into(),
            name: "金融".into(),
            description: None,
            base_output: 140.0,
            base_cost: 85.0,
            price_sensitivity: 0.5,
            employment: 90.0,
            dependencies: vec![SectorDependency {
                sector: "services".into(),
                category: Some(IndustryCategory::Tertiary),
                requirement: 0.8,
                elasticity: 1.4,
                dependency: DependencyKind::Demand,
            }],
        };

        let service_id = SectorId::new(IndustryCategory::Tertiary, "services");
        let mut metrics = HashMap::new();
        metrics.insert(
            service_id.clone(),
            SectorMetrics {
                output: 120.0,
                revenue: 0.0,
                cost: 0.0,
                sales: 100.0,
                demand: 220.0,
                inventory: 0.0,
                unmet_demand: 40.0,
            },
        );

        let states: HashMap<SectorId, SectorState> = HashMap::new();
        let impact = evaluate_dependency_impacts(
            IndustryCategory::Tertiary,
            &finance_def,
            &metrics,
            &states,
        );
        assert!(impact.demand_multiplier > 1.0);
    }
}
