use rand::rngs::StdRng;

use crate::game::CountryState;
use crate::game::economy::{ExpenseKind, IndustryTickOutcome, RevenueKind};
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

    pub fn apply_industry_outcome(
        &mut self,
        outcome: &IndustryTickOutcome,
        countries: &mut [CountryState],
    ) {
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
}
