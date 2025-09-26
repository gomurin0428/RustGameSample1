use serde::{Deserialize, Serialize};

const HOURS_PER_YEAR: f64 = 24.0 * 365.0;
const DEBT_CYCLE_PER_YEAR: f64 = 12.0;

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
pub struct DebtCycleOutcome {
    pub interest_due: f64,
    pub interest_paid: f64,
    pub principal_repaid: f64,
    pub new_issuance: f64,
    pub downgraded: Option<CreditRating>,
    pub crisis: Option<String>,
}

pub(crate) fn downgrade_rating(rating: CreditRating) -> CreditRating {
    use CreditRating::*;
    match rating {
        AAA => AA,
        AA => A,
        A => BBB,
        BBB => BB,
        BB => B,
        B => CCC,
        CCC => CC,
        CC => C,
        C => D,
        D => D,
    }
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

    pub fn update_fiscal_cycle(&mut self, gdp: f64) -> DebtCycleOutcome {
        if !gdp.is_finite() || gdp < 0.0 {
            panic!("update_fiscal_cycle に不正な GDP が渡されました");
        }

        let mut debt_ratio = if gdp > 0.0 {
            (self.debt / gdp).max(0.0)
        } else if self.debt > 0.0 {
            5.0
        } else {
            0.0
        };

        let base_rate = self.credit_rating.base_interest_rate();
        let risk_surcharge = (debt_ratio - 0.6).max(0.0) * 0.03;
        self.interest_rate = (base_rate + risk_surcharge).min(0.30);

        let interest_due = self.debt * self.interest_rate / DEBT_CYCLE_PER_YEAR;
        let mut interest_paid = 0.0;
        let mut unpaid_interest = 0.0;
        if interest_due > 0.0 {
            let payable = self.cash_reserve.min(interest_due);
            if payable > 0.0 {
                self.record_expense(ExpenseKind::DebtService, payable);
                interest_paid = payable;
            }
            unpaid_interest = interest_due - payable;
            if unpaid_interest > 0.0 {
                self.debt += unpaid_interest;
            }
        }

        let mut principal_repaid = 0.0;
        if self.debt > 0.0 {
            let amort_target = (self.debt * 0.01).min(self.cash_reserve * 0.5);
            if amort_target > 0.0 {
                self.record_expense(ExpenseKind::DebtService, amort_target);
                self.debt = (self.debt - amort_target).max(0.0);
                principal_repaid = amort_target;
            }
        }

        let safety_reserve = (gdp * 0.04).max(25.0);
        let mut new_issuance = 0.0;
        if self.cash_reserve < safety_reserve {
            let needed = safety_reserve - self.cash_reserve;
            if needed > 0.0 {
                self.add_debt(needed);
                self.record_revenue(RevenueKind::Other, needed);
                new_issuance = needed;
            }
        }

        debt_ratio = if gdp > 0.0 {
            (self.debt / gdp).max(0.0)
        } else if self.debt > 0.0 {
            5.0
        } else {
            0.0
        };

        let mut downgraded = None;
        let mut crisis = None;

        if debt_ratio > 1.1 || unpaid_interest > interest_due * 0.25 {
            let previous = self.credit_rating;
            let new_rating = downgrade_rating(previous);
            if new_rating != previous {
                self.set_credit_rating(new_rating);
                downgraded = Some(new_rating);
                crisis = Some(format!(
                    "債務比率が {:.0}% に達し、信用格付けが {:?} から {:?} に低下しました。",
                    (debt_ratio * 100.0).round(),
                    previous,
                    new_rating
                ));
            } else {
                crisis = Some(format!(
                    "債務比率が {:.0}% に達し、危機的水準です。",
                    (debt_ratio * 100.0).round()
                ));
            }
        }

        DebtCycleOutcome {
            interest_due,
            interest_paid,
            principal_repaid,
            new_issuance,
            downgraded,
            crisis,
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn update_fiscal_cycle_pays_interest_and_reduces_debt() {
        let mut account = FiscalAccount::new(300.0, CreditRating::BBB);
        account.debt = 1_200.0;
        let outcome = account.update_fiscal_cycle(1_800.0);
        assert!(outcome.interest_due > 0.0);
        assert!(outcome.interest_paid > 0.0);
        assert!(outcome.principal_repaid >= 0.0);
        assert!(account.debt >= 0.0);
    }

    #[test]
    fn update_fiscal_cycle_triggers_crisis_on_excess_debt() {
        let mut account = FiscalAccount::new(50.0, CreditRating::BBB);
        account.debt = 2_500.0;
        let outcome = account.update_fiscal_cycle(1_500.0);
        assert!(outcome.crisis.is_some());
        assert!(matches!(outcome.downgraded, Some(_)));
        assert!(account.interest_rate >= account.credit_rating.base_interest_rate());
    }
}
