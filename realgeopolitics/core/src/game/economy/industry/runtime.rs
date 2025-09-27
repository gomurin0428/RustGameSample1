use std::collections::HashMap;

use anyhow::{Result, anyhow};

use super::model::{
    IndustryCatalog, IndustryCategory, IndustryTickOutcome, SectorId, SectorMetrics,
    SectorModifier, SectorOverview, SectorState,
};
use super::{Reporter, SectorMetricsStore, effects};

#[derive(Debug, Clone)]
pub struct IndustryRuntime {
    catalog: IndustryCatalog,
    states: HashMap<SectorId, SectorState>,
    modifiers: HashMap<SectorId, SectorModifier>,
    metrics_store: SectorMetricsStore,
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
            metrics_store: SectorMetricsStore::new(),
            energy_baseline_output: energy_baseline.max(1.0),
            energy_cost_index: 1.0,
        }
    }

    pub fn simulate_tick(&mut self, minutes: f64, scale: f64) -> IndustryTickOutcome {
        if scale <= 0.0 {
            return IndustryTickOutcome::default();
        }

        self.metrics_store.begin_tick();
        let mut reporter = Reporter::new();

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
                let impact = effects::evaluate_dependency_impacts(
                    category,
                    def,
                    self.metrics_store.metrics(),
                    &self.states,
                );
                let state_entry = self
                    .states
                    .entry(sector_id.clone())
                    .or_insert_with(|| SectorState::from_definition(def, category));
                let modifier = self
                    .modifiers
                    .entry(sector_id.clone())
                    .or_insert_with(SectorModifier::default);

                let subsidy = modifier.subsidy_bonus.clamp(0.0, 0.9);
                state_entry.subsidy_rate = subsidy;

                let base_demand = (def.base_output * scale).max(0.0);
                let adjusted_demand = (base_demand * impact.demand_multiplier).max(0.0);
                let adjustment_rate = (0.35 + subsidy * 0.5).clamp(0.2, 0.95);
                let smoothed_demand = (state_entry.potential_demand * (1.0 - adjustment_rate))
                    + (adjusted_demand * adjustment_rate);
                let demand_with_backlog = smoothed_demand + state_entry.unmet_demand;

                let base_capacity = state_entry.supply_capacity.max(def.base_output * 0.1);
                let efficiency_factor =
                    (state_entry.efficiency * (1.0 + modifier.efficiency_bonus)).clamp(0.1, 3.0);
                let subsidy_boost = 1.0 + subsidy * 0.6;
                let input_limit = impact.input_availability.clamp(0.0, 1.5);
                let mut cost_factor = impact.cost_multiplier;
                if category != IndustryCategory::Energy {
                    cost_factor *= self.energy_cost_index;
                }

                let capacity_limit =
                    (base_capacity * efficiency_factor * subsidy_boost * input_limit).max(0.0)
                        * scale;
                let target_output = (state_entry.last_output * (1.0 - adjustment_rate))
                    + (smoothed_demand * adjustment_rate);
                let inertia_floor = if state_entry.last_output > 0.0 {
                    state_entry.last_output * (0.4 + subsidy * 0.3)
                } else {
                    0.0
                };
                let production = capacity_limit
                    .min(target_output.max(inertia_floor))
                    .max(0.0);

                let available_supply = production + state_entry.inventory;
                let sales = available_supply.min(demand_with_backlog);
                let new_inventory = (available_supply - sales).max(0.0);
                let new_unmet = (demand_with_backlog - sales).max(0.0);

                let gap_ratio = if demand_with_backlog <= f64::EPSILON {
                    -1.0
                } else {
                    ((demand_with_backlog - sales) / demand_with_backlog).clamp(-1.5, 1.5)
                };
                let price_multiplier = effects::price_from_gap(gap_ratio, def.price_sensitivity);
                let price = (def.base_cost * price_multiplier).max(0.05);
                let unit_cost =
                    (def.base_cost * cost_factor * (1.0 - subsidy).max(0.1)).clamp(0.05, 5_000.0);
                let cost = production * unit_cost;
                let revenue = sales * price;

                state_entry.inventory = new_inventory;
                state_entry.unmet_demand = new_unmet;
                state_entry.potential_demand = smoothed_demand.max(0.0);
                state_entry.last_output = production;
                let base_capacity_update = if scale > 0.0 {
                    (capacity_limit / scale).max(def.base_output * 0.1)
                } else {
                    base_capacity
                };
                state_entry.supply_capacity =
                    (state_entry.supply_capacity * 0.9) + (base_capacity_update * 0.1);
                let utilisation = if capacity_limit > f64::EPSILON {
                    (production / capacity_limit).clamp(0.0, 1.2)
                } else {
                    0.0
                };
                let target_efficiency =
                    (1.0 + modifier.efficiency_bonus * 0.5) * (0.9 + utilisation * 0.2);
                state_entry.efficiency =
                    (state_entry.efficiency * 0.85 + target_efficiency * 0.15).clamp(0.2, 3.0);
                modifier.decay(minutes);

                if category == IndustryCategory::Energy {
                    energy_output_total += production;
                }

                let metrics = SectorMetrics {
                    output: production,
                    revenue,
                    cost,
                    sales,
                    demand: demand_with_backlog,
                    inventory: new_inventory,
                    unmet_demand: new_unmet,
                };
                self.metrics_store.record(sector_id.clone(), metrics);
                reporter.record_sector_activity(
                    &def.name,
                    production,
                    demand_with_backlog,
                    new_inventory,
                    new_unmet,
                    sales,
                );
            }

            if category == IndustryCategory::Energy {
                self.energy_cost_index = effects::update_energy_cost_index(
                    self.energy_baseline_output,
                    energy_output_total,
                );
            }
        }

        let totals = self.metrics_store.totals();
        IndustryTickOutcome {
            total_revenue: totals.revenue(),
            total_cost: totals.cost(),
            total_gdp: totals.gdp(),
            sector_metrics: self.metrics_store.snapshot(),
            reports: reporter.into_reports(),
        }
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
            let metrics = self.metrics_store.get(id);
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
        let metrics = self.metrics_store.get(id);
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
        self.metrics_store.metrics()
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
        let mut catalog = IndustryCatalog::default();
        catalog
            .insert_definition(
                IndustryCategory::Primary,
                SectorDefinition {
                    key: "grain".into(),
                    name: "穀物".into(),
                    description: None,
                    base_output: 120.0,
                    base_cost: 50.0,
                    price_sensitivity: 0.6,
                    employment: 80.0,
                    dependencies: Vec::new(),
                },
            )
            .expect("insert grain");
        let sector_id = SectorId::new(IndustryCategory::Primary, "grain");

        let mut baseline_runtime = IndustryRuntime::from_catalog(catalog.clone());
        baseline_runtime.simulate_tick(60.0, 1.0);
        let baseline_outcome = baseline_runtime.simulate_tick(60.0, 1.0);
        let baseline_metrics = baseline_outcome
            .sector_metrics
            .get(&sector_id)
            .expect("baseline metrics")
            .clone();
        let baseline_price = baseline_metrics.revenue / baseline_metrics.sales.max(1e-6);

        let mut shortage_runtime = IndustryRuntime::from_catalog(catalog);
        shortage_runtime.simulate_tick(60.0, 1.0);
        shortage_runtime.set_modifier_for_test(&sector_id, 0.0, -0.6, 180.0);
        let shortage_outcome = shortage_runtime.simulate_tick(60.0, 1.6);
        let shortage_metrics = shortage_outcome
            .sector_metrics
            .get(&sector_id)
            .expect("shortage metrics");
        assert!(shortage_metrics.unmet_demand > 0.0);
        let shortage_price = shortage_metrics.revenue / shortage_metrics.sales.max(1e-6);
        assert!(
            shortage_price > baseline_price * 1.05,
            "price should respond to demand pressure"
        );
    }

    #[test]
    fn inventory_accumulates_when_demand_drops() {
        let mut catalog = IndustryCatalog::default();
        catalog
            .insert_definition(
                IndustryCategory::Secondary,
                SectorDefinition {
                    key: "automotive".into(),
                    name: "自動車".into(),
                    description: None,
                    base_output: 180.0,
                    base_cost: 130.0,
                    price_sensitivity: 0.4,
                    employment: 120.0,
                    dependencies: Vec::new(),
                },
            )
            .expect("insert automotive");
        catalog
            .insert_definition(
                IndustryCategory::Tertiary,
                SectorDefinition {
                    key: "logistics".into(),
                    name: "物流".into(),
                    description: None,
                    base_output: 160.0,
                    base_cost: 90.0,
                    price_sensitivity: 0.5,
                    employment: 90.0,
                    dependencies: vec![SectorDependency {
                        sector: "automotive".into(),
                        category: Some(IndustryCategory::Secondary),
                        requirement: 1.0,
                        elasticity: -2.5,
                        dependency: DependencyKind::Demand,
                    }],
                },
            )
            .expect("insert logistics");
        let auto_id = SectorId::new(IndustryCategory::Secondary, "automotive");
        let logistics_id = SectorId::new(IndustryCategory::Tertiary, "logistics");
        let mut runtime = IndustryRuntime::from_catalog(catalog);
        runtime.simulate_tick(60.0, 1.0);
        if let Some(state) = runtime.states.get_mut(&logistics_id) {
            state.last_output = 400.0;
            state.potential_demand = 400.0;
            state.supply_capacity = 400.0;
            state.inventory = 0.0;
            state.unmet_demand = 0.0;
        }
        if let Some(state) = runtime.states.get_mut(&auto_id) {
            state.last_output = 800.0;
            state.potential_demand = 800.0;
            state.supply_capacity = 800.0;
        }
        runtime.set_modifier_for_test(&auto_id, 0.0, 1.2, 180.0);
        let mut outcome = runtime.simulate_tick(60.0, 1.0);
        for _ in 0..2 {
            outcome = runtime.simulate_tick(60.0, 1.0);
        }
        let metrics = outcome
            .sector_metrics
            .get(&logistics_id)
            .expect("logistics metrics");
        assert!(
            metrics.inventory > 0.0,
            "inventory should increase when downstream demand collapses (value = {:.3})",
            metrics.inventory
        );
        assert!(
            metrics.unmet_demand < 1e-6,
            "oversupply scenario should not leave unmet demand (value = {:.3})",
            metrics.unmet_demand
        );
    }

    #[test]
    fn unmet_demand_accumulates_when_capacity_constrained() {
        let mut catalog = IndustryCatalog::default();
        catalog
            .insert_definition(
                IndustryCategory::Secondary,
                SectorDefinition {
                    key: "automotive".into(),
                    name: "自動車".into(),
                    description: None,
                    base_output: 160.0,
                    base_cost: 130.0,
                    price_sensitivity: 0.5,
                    employment: 120.0,
                    dependencies: Vec::new(),
                },
            )
            .expect("insert automotive");
        let sector_id = SectorId::new(IndustryCategory::Secondary, "automotive");
        let mut runtime = IndustryRuntime::from_catalog(catalog);
        runtime.simulate_tick(60.0, 1.0);
        runtime.set_modifier_for_test(&sector_id, 0.0, -0.8, 300.0);
        let outcome = runtime.simulate_tick(60.0, 2.0);
        let metrics = outcome
            .sector_metrics
            .get(&sector_id)
            .expect("automotive metrics");
        assert!(
            metrics.unmet_demand > 0.0,
            "unmet demand should accumulate under shortage"
        );
        assert!(
            metrics.inventory < 5.0,
            "inventory should not grow when supply is tight"
        );
    }

    #[test]
    fn long_run_simulation_remains_stable() {
        let catalog = IndustryCatalog::from_embedded().expect("catalog");
        let mut runtime = IndustryRuntime::from_catalog(catalog);
        for step in 0..120 {
            let outcome = runtime.simulate_tick(60.0, 1.0);
            for (id, metrics) in outcome.sector_metrics.iter() {
                assert!(
                    metrics.output.is_finite(),
                    "output must remain finite for {id:?}"
                );
                assert!(
                    metrics.inventory.is_finite(),
                    "inventory must remain finite for {id:?}"
                );
                assert!(
                    metrics.unmet_demand.is_finite(),
                    "unmet demand must remain finite for {id:?}"
                );
                assert!(metrics.output >= 0.0);
                assert!(metrics.inventory >= 0.0);
                assert!(metrics.unmet_demand >= 0.0);
            }
            assert!(runtime.energy_cost_index().is_finite());
            assert!(runtime.energy_cost_index() > 0.0);
            assert!(runtime.energy_cost_index() < 2.5);
            assert!(step < 1 || outcome.total_revenue.is_finite());
        }
    }
}
