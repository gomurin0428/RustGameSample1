use crate::game::country::CountryState;
use crate::game::economy::{ExpenseKind, RevenueKind, TaxOutcome};
use crate::game::market::CommodityMarket;
use crate::game::systems::diplomacy;
use crate::game::{MAX_METRIC, MAX_RESOURCES, MIN_METRIC, MIN_RESOURCES};

pub(crate) fn prepare_all_fiscal_flows(countries: &mut [CountryState], scale: f64) {
    if scale <= 0.0 {
        return;
    }
    for country in countries.iter_mut() {
        country.fiscal_mut().clear_flows();
        country.fiscal_mut().accrue_interest_hours(scale);
    }
}

pub(crate) fn apply_budget_effects(
    countries: &mut [CountryState],
    commodity_market: &CommodityMarket,
    idx: usize,
    scale: f64,
) -> Vec<String> {
    let mut reports = Vec::new();
    if idx >= countries.len() || scale <= 0.0 {
        return reports;
    }

    let employment_ratio = estimate_employment_ratio(countries, idx);
    let (gdp, resources) = {
        let country = &countries[idx];
        (country.gdp, country.resources)
    };
    let TaxOutcome {
        immediate,
        deferred,
    } = {
        let country = &mut countries[idx];
        country
            .tax_policy_mut()
            .collect(gdp, employment_ratio, scale)
    };
    if immediate > 0.0 {
        let country = &mut countries[idx];
        country
            .fiscal_mut()
            .record_revenue(RevenueKind::Taxation, immediate);
        reports.push(format!(
            "{} は税収を確保しました (即時 {:.1})",
            country.name, immediate
        ));
    }
    if deferred > 0.0 {
        reports.push(format!(
            "{} は将来計上予定の税収 {:.1} を繰越します。",
            countries[idx].name, deferred
        ));
    }

    let allocation = countries[idx].allocations();
    let gdp_amount = gdp.max(0.0);
    let percent_to_amount = |percent: f64| -> f64 {
        if percent <= 0.0 || gdp_amount <= 0.0 {
            0.0
        } else {
            gdp_amount * (percent / 100.0)
        }
    };
    let resource_revenue = commodity_market.revenue_for(resources, scale);
    if resource_revenue > 0.0 {
        let price_snapshot = commodity_market.price();
        let country = &mut countries[idx];
        country
            .fiscal_mut()
            .record_revenue(RevenueKind::ResourceExport, resource_revenue);
        reports.push(format!(
            "{} は資源輸出で {:.1} の外貨収入を獲得しました (単価 {:.1})",
            country.name, resource_revenue, price_snapshot
        ));
    }

    let debt_base = percent_to_amount(allocation.debt_service);
    let debt_request = if allocation.ensure_core_minimum {
        debt_base.max(essential_debt_target(countries, idx))
    } else {
        debt_base
    };
    let debt_desired = debt_request * scale;
    if debt_desired > 0.0 {
        let available = countries[idx].cash_reserve();
        let actual = debt_desired.min(available);
        if actual > 0.0 {
            let country = &mut countries[idx];
            country
                .fiscal_mut()
                .record_expense(ExpenseKind::DebtService, actual);
            let reduction = actual.min(country.fiscal_mut().debt);
            if reduction > 0.0 {
                country.fiscal_mut().add_debt(-reduction);
            }
            reports.push(format!(
                "{} は債務返済に {:.1} を充当しました。",
                country.name, actual
            ));
        } else if allocation.ensure_core_minimum {
            let country = &mut countries[idx];
            country.fiscal_mut().add_debt(debt_desired * 0.25);
            reports.push(format!(
                "{} は債務返済資金が不足し、返済を繰り延べました。",
                country.name
            ));
        }
    }
    let administration_base = percent_to_amount(allocation.administration);
    let administration_request = if allocation.ensure_core_minimum {
        administration_base.max(essential_administration_target(countries, idx))
    } else {
        administration_base
    };
    let administration_desired = administration_request * scale;
    if administration_desired > 0.0 {
        let available = countries[idx].cash_reserve();
        let actual = administration_desired.min(available);
        if actual > 0.0 {
            let country = &mut countries[idx];
            country
                .fiscal_mut()
                .record_expense(ExpenseKind::Administration, actual);
            let stability_gain = (actual / 120.0).round() as i32;
            country.stability = clamp_metric(country.stability + stability_gain);
            reports.push(format!(
                "{} は行政維持に {:.1} を投じています。",
                country.name, actual
            ));
        } else if allocation.ensure_core_minimum {
            let country = &mut countries[idx];
            country.stability = clamp_metric(country.stability - 3);
            reports.push(format!(
                "{} は行政費の不足で行政効率が低下しています。",
                country.name
            ));
        }
    }

    let infra_desired = percent_to_amount(allocation.infrastructure) * scale;
    if infra_desired > 0.0 {
        let available = countries[idx].cash_reserve();
        let actual = infra_desired.min(available);
        if actual > 0.0 {
            let country = &mut countries[idx];
            country
                .fiscal_mut()
                .record_expense(ExpenseKind::Infrastructure, actual);
            country.gdp += actual * 0.9;
            let intensity = (actual / 80.0).round() as i32;
            country.stability = clamp_metric(country.stability + intensity);
            country.approval = clamp_metric(country.approval + (intensity / 2));
            country.resources = clamp_resource(country.resources - (actual / 25.0) as i32);
            reports.push(format!(
                "{} がインフラ投資を実施中です (支出 {:.1})",
                country.name, actual
            ));
        }
    }
    let welfare_desired = percent_to_amount(allocation.welfare) * scale;
    if welfare_desired > 0.0 {
        let available = countries[idx].cash_reserve();
        let actual = welfare_desired.min(available);
        if actual > 0.0 {
            let country = &mut countries[idx];
            country
                .fiscal_mut()
                .record_expense(ExpenseKind::Welfare, actual);
            let intensity = (actual / 70.0).round() as i32;
            country.approval = clamp_metric(country.approval + intensity);
            country.stability = clamp_metric(country.stability + (intensity / 2));
            country.gdp = (country.gdp - actual * 0.25).max(0.0);
            reports.push(format!(
                "{} が社会福祉を拡充しました (支出 {:.1})",
                country.name, actual
            ));
        }
    }

    let research_desired = percent_to_amount(allocation.research) * scale;
    if research_desired > 0.0 {
        let available = countries[idx].cash_reserve();
        let actual = research_desired.min(available);
        if actual > 0.0 {
            let country = &mut countries[idx];
            country
                .fiscal_mut()
                .record_expense(ExpenseKind::Research, actual);
            country.gdp += actual * 0.6;
            let innovation = (actual / 90.0).round() as i32;
            country.resources = clamp_resource(country.resources + innovation);
            reports.push(format!(
                "{} は研究開発に {:.1} を投資しました。",
                country.name, actual
            ));
        }
    }

    let diplomacy_desired = percent_to_amount(allocation.diplomacy) * scale;
    if diplomacy_desired > 0.0 {
        let available = countries[idx].cash_reserve();
        let actual = diplomacy_desired.min(available);
        if actual > 0.0 {
            let country_name = countries[idx].name.clone();
            {
                let country = &mut countries[idx];
                country
                    .fiscal_mut()
                    .record_expense(ExpenseKind::Diplomacy, actual);
            }
            let relation_scale = (actual / 120.0).max(scale);
            diplomacy::improve_relations(countries, idx, relation_scale);
            reports.push(format!(
                "{} が外交関係の改善に取り組んでいます (支出 {:.1})",
                country_name, actual
            ));
        }
    }

    let military_desired = percent_to_amount(allocation.military) * scale;
    if military_desired > 0.0 {
        let available = countries[idx].cash_reserve();
        let actual = military_desired.min(available);
        if actual > 0.0 {
            let country_name = countries[idx].name.clone();
            {
                let country = &mut countries[idx];
                country
                    .fiscal_mut()
                    .record_expense(ExpenseKind::Military, actual);
                let intensity = (actual / 80.0).round() as i32;
                country.military = clamp_metric(country.military + intensity);
                country.stability = clamp_metric(country.stability + (intensity / 2));
                country.approval = clamp_metric(country.approval - (intensity / 2));
                country.resources = clamp_resource(country.resources - (actual / 40.0) as i32);
            }
            let relation_penalty = -((2.0 * scale.max(1.0)).round() as i32);
            diplomacy::penalise_after_military(countries, idx, relation_penalty);
            reports.push(format!(
                "{} が軍事強化に予算を充当しました (支出 {:.1})",
                country_name, actual
            ));
        }
    }

    reports
}
fn estimate_employment_ratio(countries: &[CountryState], idx: usize) -> f64 {
    countries
        .get(idx)
        .map(|country| {
            let stability_factor = country.stability as f64 / MAX_METRIC as f64;
            let approval_factor = country.approval as f64 / MAX_METRIC as f64;
            ((stability_factor * 0.6) + (approval_factor * 0.4)).clamp(0.4, 1.2)
        })
        .unwrap_or(0.9)
}

fn essential_debt_target(countries: &[CountryState], idx: usize) -> f64 {
    let country = &countries[idx];
    (country.fiscal.debt * country.fiscal.interest_rate / 24.0).clamp(50.0, 300.0)
}

fn essential_administration_target(countries: &[CountryState], idx: usize) -> f64 {
    let country = &countries[idx];
    (country.population_millions * 2.0).max(35.0)
}

fn clamp_metric(value: i32) -> i32 {
    value.clamp(MIN_METRIC, MAX_METRIC)
}

fn clamp_resource(value: i32) -> i32 {
    value.clamp(MIN_RESOURCES, MAX_RESOURCES)
}
