#[cfg(test)]
use std::collections::HashMap;

use anyhow::Result;

use crate::game::country::CountryState;
#[cfg(test)]
use crate::game::economy::SectorId;
#[cfg(test)]
use crate::game::economy::industry::SectorMetrics;
use crate::game::economy::{
    ExpenseKind, IndustryRuntime, IndustryTickOutcome, RevenueKind, SectorOverview,
};

pub(crate) struct IndustryEngine {
    runtime: IndustryRuntime,
}

impl IndustryEngine {
    pub fn new(runtime: IndustryRuntime) -> Self {
        Self { runtime }
    }

    pub fn overview(&self) -> Vec<SectorOverview> {
        self.runtime.overview()
    }

    pub fn apply_industry_subsidy(&mut self, token: &str, percent: f64) -> Result<SectorOverview> {
        let id = self.runtime.resolve_sector_token(token)?;
        self.runtime.apply_subsidy(&id, percent)
    }

    pub fn simulate_tick(
        &mut self,
        minutes: f64,
        scale: f64,
        countries: &mut [CountryState],
    ) -> IndustryTickOutcome {
        let outcome = self.runtime.simulate_tick(minutes, scale);
        self.distribute_outcome(&outcome, countries);
        outcome
    }

    fn distribute_outcome(&self, outcome: &IndustryTickOutcome, countries: &mut [CountryState]) {
        let count = countries.len();
        if count == 0 {
            return;
        }
        let per_country = count as f64;
        let revenue_share = outcome.total_revenue / per_country;
        let cost_share = outcome.total_cost / per_country;
        let gdp_share = outcome.total_gdp / per_country;
        for country in countries.iter_mut() {
            if revenue_share > 0.0 {
                country
                    .fiscal_mut()
                    .record_revenue(RevenueKind::Trade, revenue_share);
            }
            if cost_share > 0.0 {
                country
                    .fiscal_mut()
                    .record_expense(ExpenseKind::IndustrySupport, cost_share);
            }
            if gdp_share.abs() > f64::EPSILON {
                country.gdp = (country.gdp + gdp_share).max(0.0);
            }
        }
    }

    #[cfg(test)]
    pub(crate) fn metrics(&self) -> &HashMap<SectorId, SectorMetrics> {
        self.runtime.metrics()
    }

    #[cfg(test)]
    pub(crate) fn set_modifier_for_test(
        &mut self,
        id: &SectorId,
        subsidy_bonus: f64,
        efficiency_bonus: f64,
        duration_minutes: f64,
    ) {
        self.runtime
            .set_modifier_for_test(id, subsidy_bonus, efficiency_bonus, duration_minutes);
    }

    #[cfg(test)]
    pub(crate) fn distribute_outcome_for_test(
        &self,
        outcome: &IndustryTickOutcome,
        countries: &mut [CountryState],
    ) {
        self.distribute_outcome(outcome, countries);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::economy::{CreditRating, FiscalAccount, IndustryCatalog, TaxPolicy};
    use crate::game::{BudgetAllocation, CountryState, IndustryCategory};

    fn sample_country(name: &str) -> CountryState {
        CountryState::new(
            name.to_string(),
            "Republic".to_string(),
            30.0,
            1500.0,
            60,
            55,
            50,
            70,
            FiscalAccount::new(300.0, CreditRating::BBB),
            TaxPolicy::default(),
            BudgetAllocation::default(),
        )
    }

    #[test]
    fn distribute_outcome_allocates_per_country() {
        let catalog = IndustryCatalog::from_embedded().expect("catalog");
        let runtime = IndustryRuntime::from_catalog(catalog);
        let engine = IndustryEngine::new(runtime);

        let mut countries = vec![sample_country("Asteria"), sample_country("Borealis")];
        let mut outcome = IndustryTickOutcome::default();
        outcome.total_revenue = 200.0;
        outcome.total_cost = 60.0;
        outcome.total_gdp = 40.0;

        let baseline: Vec<(f64, f64, f64)> = countries
            .iter()
            .map(|c| (c.total_revenue(), c.total_expense(), c.gdp))
            .collect();

        engine.distribute_outcome_for_test(&outcome, &mut countries);

        let share_revenue = outcome.total_revenue / countries.len() as f64;
        let share_cost = outcome.total_cost / countries.len() as f64;
        let share_gdp = outcome.total_gdp / countries.len() as f64;

        for (idx, country) in countries.iter().enumerate() {
            assert!(country.total_revenue() >= baseline[idx].0 + share_revenue * 0.99);
            assert!(country.total_expense() >= baseline[idx].1 + share_cost * 0.99);
            assert!((country.gdp - (baseline[idx].2 + share_gdp)).abs() < 1e-6);
        }
    }

    #[test]
    fn apply_industry_subsidy_resolves_token() {
        let catalog = IndustryCatalog::from_embedded().expect("catalog");
        let runtime = IndustryRuntime::from_catalog(catalog);
        let mut engine = IndustryEngine::new(runtime);
        let mut countries = vec![sample_country("Asteria")];
        engine.simulate_tick(60.0, 1.0, countries.as_mut_slice());
        let overview = engine
            .apply_industry_subsidy("energy:electricity", 20.0)
            .expect("subsidy");
        assert_eq!(overview.id.category, IndustryCategory::Energy);
    }

    #[test]
    fn metrics_forward_to_runtime() {
        let catalog = IndustryCatalog::from_embedded().expect("catalog");
        let runtime = IndustryRuntime::from_catalog(catalog);
        let engine = IndustryEngine::new(runtime);
        assert!(engine.metrics().is_empty());
    }
}
