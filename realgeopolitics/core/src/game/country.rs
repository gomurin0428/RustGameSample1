use std::collections::HashMap;

use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};

use super::economy::{FiscalAccount, TaxPolicy, TaxPolicyConfig};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CountryDefinition {
    pub name: String,
    pub government: String,
    pub population_millions: f64,
    pub gdp: f64,
    pub stability: i32,
    pub military: i32,
    pub approval: i32,
    pub budget: f64,
    pub resources: i32,
    #[serde(default)]
    pub tax_policy: Option<TaxPolicyConfig>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct BudgetAllocation {
    pub infrastructure: f64,
    pub military: f64,
    pub welfare: f64,
    pub diplomacy: f64,
    pub debt_service: f64,
    pub administration: f64,
    pub research: f64,
    #[serde(default)]
    pub ensure_core_minimum: bool,
}
impl BudgetAllocation {
    pub fn new(
        infrastructure: f64,
        military: f64,
        welfare: f64,
        diplomacy: f64,
        debt_service: f64,
        administration: f64,
        research: f64,
        ensure_core_minimum: bool,
    ) -> Result<Self> {
        for (label, value) in [
            ("インフラ", infrastructure),
            ("軍事", military),
            ("福祉", welfare),
            ("外交", diplomacy),
            ("債務返済", debt_service),
            ("行政維持", administration),
            ("研究開発", research),
        ] {
            ensure!(value.is_finite(), "{}予算割合が不正です", label);
            ensure!(value >= 0.0, "{}予算割合は0以上で指定してください", label);
        }
        Ok(Self {
            infrastructure,
            military,
            welfare,
            diplomacy,
            debt_service,
            administration,
            research,
            ensure_core_minimum,
        })
    }
    pub fn from_values(
        infrastructure: f64,
        military: f64,
        welfare: f64,
        diplomacy: f64,
        debt_service: f64,
        administration: f64,
        research: f64,
    ) -> Result<Self> {
        Self::new(
            infrastructure,
            military,
            welfare,
            diplomacy,
            debt_service,
            administration,
            research,
            true,
        )
    }

    pub fn total_percentage(&self) -> f64 {
        self.infrastructure
            + self.military
            + self.welfare
            + self.diplomacy
            + self.debt_service
            + self.administration
            + self.research
    }

    pub fn total_requested_amount(&self, gdp: f64) -> f64 {
        let factor = (gdp.max(0.0)) / 100.0;
        factor
            * (self.infrastructure
                + self.military
                + self.welfare
                + self.diplomacy
                + self.debt_service
                + self.administration
                + self.research)
    }

    pub fn with_core_minimum(mut self, enabled: bool) -> Self {
        self.ensure_core_minimum = enabled;
        self
    }
}
impl Default for BudgetAllocation {
    fn default() -> Self {
        Self::new(8.0, 6.0, 7.0, 5.0, 5.0, 3.5, 4.5, true)
            .expect("default budget allocation must be valid")
    }
}

#[derive(Debug, Clone)]
pub struct CountryState {
    pub name: String,
    pub government: String,
    pub population_millions: f64,
    pub gdp: f64,
    pub stability: i32,
    pub military: i32,
    pub approval: i32,
    pub resources: i32,
    pub relations: HashMap<String, i32>,
    pub fiscal: FiscalAccount,
    pub tax_policy: TaxPolicy,
    allocations: BudgetAllocation,
}
impl CountryState {
    pub(crate) fn new(
        name: String,
        government: String,
        population_millions: f64,
        gdp: f64,
        stability: i32,
        military: i32,
        approval: i32,
        resources: i32,
        fiscal: FiscalAccount,
        tax_policy: TaxPolicy,
        allocations: BudgetAllocation,
    ) -> Self {
        Self {
            name,
            government,
            population_millions,
            gdp,
            stability,
            military,
            approval,
            resources,
            relations: HashMap::new(),
            fiscal,
            tax_policy,
            allocations,
        }
    }

    pub fn allocations(&self) -> BudgetAllocation {
        self.allocations
    }

    pub fn cash_reserve(&self) -> f64 {
        self.fiscal.cash_reserve()
    }

    pub fn total_revenue(&self) -> f64 {
        self.fiscal.total_revenue()
    }

    pub fn total_expense(&self) -> f64 {
        self.fiscal.total_expense()
    }

    pub fn net_cash_flow(&self) -> f64 {
        self.fiscal.net_cash_flow()
    }

    pub fn tax_policy(&self) -> &TaxPolicy {
        &self.tax_policy
    }

    pub(crate) fn set_allocations(&mut self, allocations: BudgetAllocation) {
        self.allocations = allocations;
    }

    pub(crate) fn tax_policy_mut(&mut self) -> &mut TaxPolicy {
        &mut self.tax_policy
    }

    #[cfg(test)]
    pub fn fiscal_mut(&mut self) -> &mut FiscalAccount {
        &mut self.fiscal
    }
}
