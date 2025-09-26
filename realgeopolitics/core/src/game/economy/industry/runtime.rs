use std::collections::HashMap;

use anyhow::{Result, anyhow};

use super::effects;
use super::model::{
    IndustryCatalog, IndustryCategory, IndustryTickOutcome, SectorId, SectorMetrics,
    SectorModifier, SectorOverview, SectorState,
};

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
        if scale <= 0.0 {
            return IndustryTickOutcome::default();
        }

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
                    effects::compute_dependency_effects(category, def, &outcome.sector_metrics);
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
                let price_multiplier = effects::sigmoid_price(demand_signal, def.price_sensitivity);
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
                self.energy_cost_index = effects::update_energy_cost_index(
                    self.energy_baseline_output,
                    energy_output_total,
                );
            }
        }
        self.last_metrics = outcome.sector_metrics.clone();
        outcome
    }

    pub fn resolve_sector_token(&self, token: &str) -> Result<SectorId> {
        effects::resolve_sector_token(&self.catalog, token)
    }

    pub fn apply_subsidy(&mut self, id: &SectorId, percent: f64) -> Result<SectorOverview> {
        effects::apply_subsidy(
            &self.catalog,
            &mut self.modifiers,
            &mut self.states,
            id,
            percent,
        )?;
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

    pub fn metrics(&self) -> &HashMap<SectorId, SectorMetrics> {
        &self.last_metrics
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::economy::{
        DependencyKind, IndustryCatalog, IndustryCategory, SectorDefinition, SectorDependency,
        SectorId,
    };

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
        catalog
            .insert_definition(
                IndustryCategory::Secondary,
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
            )
            .expect("insert automotive");

        let mut baseline_runtime = IndustryRuntime::from_catalog(catalog.clone());
        let baseline_output = baseline_runtime
            .simulate_tick(60.0, 1.0)
            .sector_metrics
            .get(&SectorId::new(IndustryCategory::Secondary, "automotive"))
            .expect("baseline automotive metrics")
            .output;

        let mut shortage_runtime = IndustryRuntime::from_catalog(catalog);
        shortage_runtime.set_modifier_for_test(
            &SectorId::new(IndustryCategory::Energy, "electricity"),
            0.0,
            -0.9,
            120.0,
        );
        let shortage_output = shortage_runtime
            .simulate_tick(60.0, 1.0)
            .sector_metrics
            .get(&SectorId::new(IndustryCategory::Secondary, "automotive"))
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
        let finance_id = SectorId::new(IndustryCategory::Tertiary, "finance");
        if let Some(def) = catalog.get_mut(&finance_id) {
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
        let metrics = outcome
            .sector_metrics
            .get(&finance_id)
            .expect("finance metrics");
        assert!(metrics.revenue >= metrics.cost);
    }
}
