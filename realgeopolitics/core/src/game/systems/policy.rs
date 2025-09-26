use crate::game::country::CountryState;
use crate::game::economy::{RevenueKind, downgrade_rating};
use crate::game::{MAX_METRIC, MIN_METRIC};

pub(crate) fn resolve(countries: &mut [CountryState]) -> Vec<String> {
    let mut reports = Vec::new();
    for idx in 0..countries.len() {
        let allocation = countries[idx].allocations();
        let gdp = countries[idx].gdp.max(0.0);

        if allocation.ensure_core_minimum {
            let min_debt = (countries[idx].fiscal.debt * countries[idx].fiscal.interest_rate
                / 360.0)
                .max(40.0);
            let allocated_debt = (gdp * (allocation.debt_service / 100.0)).max(0.0);
            if allocated_debt + f64::EPSILON < min_debt {
                let country = &mut countries[idx];
                country.fiscal.add_debt(min_debt * 0.2);
                let downgraded = downgrade_rating(country.fiscal.credit_rating);
                country.fiscal.set_credit_rating(downgraded);
                reports.push(format!(
                    "{} は債務返済が不足し、信用格付けが低下しました。",
                    country.name
                ));
            }

            let admin_target = essential_administration_target(&countries[idx]);
            let allocated_admin = (gdp * (allocation.administration / 100.0)).max(0.0);
            if allocated_admin + f64::EPSILON < admin_target {
                let country = &mut countries[idx];
                country.stability = clamp_metric(country.stability - 2);
                reports.push(format!(
                    "{} は行政維持費が不足し、行政効率が悪化しています。",
                    country.name
                ));
            }
        }

        {
            let country = &mut countries[idx];
            let requested = allocation.total_requested_amount(gdp);
            let reserve_bonus = (requested * 0.05).min(country.fiscal.cash_reserve() * 0.02);
            if reserve_bonus > 0.0 {
                country
                    .fiscal
                    .record_revenue(RevenueKind::Other, reserve_bonus);
                reports.push(format!(
                    "{} は予備費を {:.1} 積み増しました。",
                    country.name, reserve_bonus
                ));
            }
        }

        if countries[idx].resources < 25 {
            let country = &mut countries[idx];
            country.gdp = (country.gdp - 20.0).max(0.0);
            reports.push(format!(
                "{} は資源不足で生産が停滞しています。",
                country.name
            ));
        }

        let outcome = {
            let country = &mut countries[idx];
            country.fiscal_mut().update_fiscal_cycle(gdp)
        };
        if outcome.interest_paid > 0.0 {
            reports.push(format!(
                "{} は利払いとして {:.1} を支出しました。",
                countries[idx].name, outcome.interest_paid
            ));
        }
        if outcome.principal_repaid > 0.0 {
            reports.push(format!(
                "{} は元本償還に {:.1} を充当しました。",
                countries[idx].name, outcome.principal_repaid
            ));
        }
        if outcome.new_issuance > 0.0 {
            reports.push(format!(
                "{} は新たに {:.1} を起債し、流動性を確保しました。",
                countries[idx].name, outcome.new_issuance
            ));
        }
        if let Some(new_rating) = outcome.downgraded {
            reports.push(format!(
                "{} の信用格付けは {:?} に引き下げられました。",
                countries[idx].name, new_rating
            ));
        }
        if let Some(alert) = outcome.crisis {
            reports.push(alert);
        }
    }

    reports
}

fn essential_administration_target(country: &CountryState) -> f64 {
    (country.population_millions * 2.0).max(35.0)
}

fn clamp_metric(value: i32) -> i32 {
    value.clamp(MIN_METRIC, MAX_METRIC)
}
