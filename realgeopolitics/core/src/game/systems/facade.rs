use rand::rngs::StdRng;

use crate::game::CountryState;
use crate::game::market::CommodityMarket;

use super::{diplomacy, events, fiscal, policy};

pub(crate) struct SystemsFacade {
    fiscal_prepared: bool,
}

impl SystemsFacade {
    pub fn new() -> Self {
        Self {
            fiscal_prepared: false,
        }
    }

    pub fn ensure_fiscal_prepared(&mut self, countries: &mut [CountryState], scale: f64) -> bool {
        if self.fiscal_prepared {
            return false;
        }
        fiscal::prepare_all_fiscal_flows(countries, scale);
        self.fiscal_prepared = true;
        true
    }

    pub fn finish_fiscal_cycle(&mut self) {
        self.fiscal_prepared = false;
    }

    pub fn apply_country_systems(
        &mut self,
        countries: &mut [CountryState],
        commodity_market: &CommodityMarket,
        rng: &mut StdRng,
        idx: usize,
        scale: f64,
    ) -> Vec<String> {
        let mut reports = fiscal::apply_budget_effects(countries, commodity_market, idx, scale);
        if let Some(event_report) = events::trigger_random_event(countries, rng, idx, scale) {
            reports.push(event_report);
        }
        if let Some(drift_report) = events::apply_economic_drift(countries, idx, scale) {
            reports.push(drift_report);
        }
        reports
    }

    pub fn process_event_trigger(&mut self, countries: &mut [CountryState]) -> Vec<String> {
        events::process_event_trigger(countries)
    }

    pub fn process_policy_resolution(&mut self, countries: &mut [CountryState]) -> Vec<String> {
        policy::resolve(countries)
    }

    pub fn process_diplomatic_pulse(&mut self, countries: &mut [CountryState]) -> Vec<String> {
        diplomacy::pulse(countries)
    }

    pub fn process_economic_tick(
        &mut self,
        countries: &mut [CountryState],
        commodity_market: &CommodityMarket,
        rng: &mut StdRng,
        scale: f64,
    ) -> Vec<String> {
        let already_prepared = self.fiscal_prepared;
        if !already_prepared {
            fiscal::prepare_all_fiscal_flows(countries, scale);
            self.fiscal_prepared = true;
        }

        let mut reports = Vec::new();
        for idx in 0..countries.len() {
            reports.extend(self.apply_country_systems(
                countries,
                commodity_market,
                rng,
                idx,
                scale,
            ));
        }

        if !already_prepared {
            self.fiscal_prepared = false;
        }
        reports
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;

    use crate::game::economy::{CreditRating, FiscalAccount, TaxPolicy};
    use crate::game::market::CommodityMarket;
    use crate::game::{BudgetAllocation, CountryState};

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
    fn ensure_fiscal_prepared_tracks_state() {
        let mut facade = SystemsFacade::new();
        let mut countries = vec![sample_country("Asteria")];

        assert!(facade.ensure_fiscal_prepared(&mut countries, 1.0));
        assert!(!facade.ensure_fiscal_prepared(&mut countries, 1.0));

        facade.finish_fiscal_cycle();
        assert!(facade.ensure_fiscal_prepared(&mut countries, 1.0));
    }

    #[test]
    fn process_economic_tick_resets_preparation_when_not_prepared() {
        let mut facade = SystemsFacade::new();
        let mut countries = vec![sample_country("Asteria"), sample_country("Borealis")];
        let market = CommodityMarket::new(120.0, 7.5, 0.04);
        let mut rng = SeedableRng::seed_from_u64(7);

        let reports = facade.process_economic_tick(&mut countries, &market, &mut rng, 1.0);
        assert!(facade.ensure_fiscal_prepared(&mut countries, 1.0));
        assert!(reports.len() >= countries.len());
    }

    #[test]
    fn process_economic_tick_preserves_prepared_state_when_already_prepared() {
        let mut facade = SystemsFacade::new();
        let mut countries = vec![sample_country("Asteria"), sample_country("Borealis")];
        let market = CommodityMarket::new(120.0, 7.5, 0.04);
        let mut rng = SeedableRng::seed_from_u64(11);

        assert!(facade.ensure_fiscal_prepared(&mut countries, 1.0));
        let _ = facade.process_economic_tick(&mut countries, &market, &mut rng, 1.0);
        assert!(!facade.ensure_fiscal_prepared(&mut countries, 1.0));
    }
}
