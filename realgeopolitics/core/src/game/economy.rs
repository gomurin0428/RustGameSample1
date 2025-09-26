use serde::{Deserialize, Serialize};

const HOURS_PER_YEAR: f64 = 24.0 * 365.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CreditRating {
    AAA,
    AA,
    A,
    BBB,
    BB,
    B,
    CCC,
    CC,
    C,
    D,
}

impl CreditRating {
    pub fn base_interest_rate(self) -> f64 {
        match self {
            CreditRating::AAA => 0.02,
            CreditRating::AA => 0.025,
            CreditRating::A => 0.03,
            CreditRating::BBB => 0.035,
            CreditRating::BB => 0.04,
            CreditRating::B => 0.05,
            CreditRating::CCC => 0.065,
            CreditRating::CC => 0.08,
            CreditRating::C => 0.1,
            CreditRating::D => 0.18,
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RevenueKind {
    Taxation,
    ResourceExport,
    Trade,
    Aid,
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExpenseKind {
    Infrastructure,
    Military,
    Welfare,
    Diplomacy,
    DebtService,
    Administration,
    Research,
    Other,
}

#[derive(Debug, Clone)]
pub struct RevenueSource {
    pub kind: RevenueKind,
    pub amount: f64,
}

#[derive(Debug, Clone)]
pub struct ExpenseItem {
    pub kind: ExpenseKind,
    pub amount: f64,
}
#[derive(Debug, Clone)]
pub struct FiscalAccount {
    cash_reserve: f64,
    pub revenues: Vec<RevenueSource>,
    pub expenses: Vec<ExpenseItem>,
    pub debt: f64,
    pub interest_rate: f64,
    pub credit_rating: CreditRating,
}

impl FiscalAccount {
    pub fn new(initial_cash: f64, rating: CreditRating) -> Self {
        Self {
            cash_reserve: initial_cash.max(0.0),
            revenues: Vec::new(),
            expenses: Vec::new(),
            debt: 0.0,
            interest_rate: rating.base_interest_rate(),
            credit_rating: rating,
        }
    }

    pub fn cash_reserve(&self) -> f64 {
        self.cash_reserve
    }

    pub fn set_cash_reserve(&mut self, amount: f64) {
        self.cash_reserve = amount.max(0.0);
    }

    pub fn set_credit_rating(&mut self, rating: CreditRating) {
        self.credit_rating = rating;
        self.interest_rate = rating.base_interest_rate();
    }
    pub fn record_revenue(&mut self, kind: RevenueKind, amount: f64) {
        if amount <= 0.0 {
            return;
        }
        self.revenues.push(RevenueSource { kind, amount });
        self.cash_reserve += amount;
    }

    pub fn record_expense(&mut self, kind: ExpenseKind, amount: f64) {
        if amount <= 0.0 {
            return;
        }
        self.expenses.push(ExpenseItem { kind, amount });
        self.cash_reserve = (self.cash_reserve - amount).max(0.0);
    }

    pub fn clear_flows(&mut self) {
        self.revenues.clear();
        self.expenses.clear();
    }

    pub fn total_revenue(&self) -> f64 {
        self.revenues.iter().map(|item| item.amount).sum()
    }

    pub fn total_expense(&self) -> f64 {
        self.expenses.iter().map(|item| item.amount).sum()
    }

    pub fn net_cash_flow(&self) -> f64 {
        self.total_revenue() - self.total_expense()
    }

    pub fn accrue_interest_hours(&mut self, hours: f64) -> f64 {
        if self.debt <= 0.0 || hours <= 0.0 {
            return 0.0;
        }
        let interest = self.debt * self.interest_rate * (hours / HOURS_PER_YEAR);
        if interest > 0.0 {
            self.record_expense(ExpenseKind::DebtService, interest);
        }
        interest
    }

    pub fn add_debt(&mut self, delta: f64) {
        self.debt = (self.debt + delta).max(0.0);
    }
}
#[derive(Debug, Clone, Copy)]
pub struct TaxOutcome {
    pub immediate: f64,
    pub deferred: f64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct TaxPolicyConfig {
    #[serde(default = "TaxPolicy::default_income_rate")]
    pub income_rate: f64,
    #[serde(default = "TaxPolicy::default_corporate_rate")]
    pub corporate_rate: f64,
    #[serde(default = "TaxPolicy::default_consumption_rate")]
    pub consumption_rate: f64,
    #[serde(default)]
    pub deductions: f64,
    #[serde(default = "TaxPolicy::default_gdp_sensitivity")]
    pub gdp_sensitivity: f64,
    #[serde(default = "TaxPolicy::default_employment_sensitivity")]
    pub employment_sensitivity: f64,
}

#[derive(Debug, Clone)]
pub struct TaxPolicy {
    pub income_rate: f64,
    pub corporate_rate: f64,
    pub consumption_rate: f64,
    pub deductions: f64,
    pub gdp_sensitivity: f64,
    pub employment_sensitivity: f64,
    lagged_revenue: f64,
}

impl TaxPolicy {
    const MIN_RATE: f64 = 0.0;
    const MAX_RATE: f64 = 0.6;
    pub fn default_income_rate() -> f64 {
        0.18
    }

    pub fn default_corporate_rate() -> f64 {
        0.22
    }

    pub fn default_consumption_rate() -> f64 {
        0.08
    }

    pub fn default_gdp_sensitivity() -> f64 {
        0.25
    }

    pub fn default_employment_sensitivity() -> f64 {
        0.2
    }

    pub fn new(config: TaxPolicyConfig) -> Self {
        Self {
            income_rate: config.income_rate.clamp(Self::MIN_RATE, Self::MAX_RATE),
            corporate_rate: config.corporate_rate.clamp(Self::MIN_RATE, Self::MAX_RATE),
            consumption_rate: config
                .consumption_rate
                .clamp(Self::MIN_RATE, Self::MAX_RATE),
            deductions: config.deductions.max(0.0),
            gdp_sensitivity: config.gdp_sensitivity.clamp(-1.0, 1.0),
            employment_sensitivity: config.employment_sensitivity.clamp(-1.0, 1.0),
            lagged_revenue: 0.0,
        }
    }
    pub fn default() -> Self {
        Self::new(TaxPolicyConfig {
            income_rate: Self::default_income_rate(),
            corporate_rate: Self::default_corporate_rate(),
            consumption_rate: Self::default_consumption_rate(),
            deductions: 0.0,
            gdp_sensitivity: Self::default_gdp_sensitivity(),
            employment_sensitivity: Self::default_employment_sensitivity(),
        })
    }

    pub fn collect(&mut self, gdp: f64, employment_ratio: f64, scale: f64) -> TaxOutcome {
        let gdp_scaled = gdp.max(0.0);
        let income_base = gdp_scaled * 0.45 * self.income_rate;
        let corporate_base = gdp_scaled * 0.35 * self.corporate_rate;
        let consumption_base = gdp_scaled * 0.20 * self.consumption_rate;
        let gross = income_base + corporate_base + consumption_base;
        let deduction = self.deductions.min(gross * 0.4);
        let structural = (gross - deduction).max(0.0);

        let gdp_factor = 1.0 + self.gdp_sensitivity * ((gdp_scaled / 1500.0) - 1.0);
        let employment_factor = 1.0 + self.employment_sensitivity * (employment_ratio - 0.9);
        let adjusted = (structural * gdp_factor * employment_factor).max(0.0) * scale;

        let immediate = (adjusted * 0.7) + self.lagged_revenue;
        let deferred = adjusted * 0.3;
        self.lagged_revenue = deferred;

        TaxOutcome {
            immediate,
            deferred,
        }
    }

    pub fn pending_revenue(&self) -> f64 {
        self.lagged_revenue
    }
}
