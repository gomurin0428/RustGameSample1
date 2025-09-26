use rand::Rng;
use rand::rngs::StdRng;

use crate::game::country::CountryState;
use crate::game::{MAX_METRIC, MAX_RESOURCES, MIN_METRIC, MIN_RESOURCES};

pub(crate) fn process_event_trigger(countries: &mut [CountryState]) -> Vec<String> {
    let mut reports = Vec::new();
    for country in countries.iter_mut() {
        if country.stability < 35 {
            country.approval = clamp_metric(country.approval - 2);
            reports.push(format!(
                "{} で治安不安が高まり、国民支持が低下しました。",
                country.name
            ));
        } else if country.approval < 30 {
            country.stability = clamp_metric(country.stability - 1);
            reports.push(format!(
                "{} では抗議活動が発生し、安定度がわずかに悪化しました。",
                country.name
            ));
        }
    }
    reports
}

pub(crate) fn trigger_random_event(
    countries: &mut [CountryState],
    rng: &mut StdRng,
    idx: usize,
    scale: f64,
) -> Option<String> {
    let probability = (0.25 * scale).clamp(0.0, 1.0);
    if !rng.gen_bool(probability as f64) {
        return None;
    }

    let country = &mut countries[idx];
    match rng.gen_range(0..3) {
        0 => {
            country.gdp += 60.0 * scale;
            country.approval = clamp_metric(country.approval + (2.0 * scale) as i32);
            Some(format!(
                "{} で技術革新が発生し、経済が加速しました。",
                country.name
            ))
        }
        1 => {
            country.stability = clamp_metric(country.stability - (5.0 * scale) as i32);
            country.approval = clamp_metric(country.approval - (4.0 * scale) as i32);
            Some(format!(
                "{} で抗議運動が拡大し、安定度が低下しました。",
                country.name
            ))
        }
        2 => {
            country.resources = clamp_resource(country.resources - (6.0 * scale) as i32);
            country.military = clamp_metric(country.military + (3.0 * scale) as i32);
            Some(format!(
                "{} は国境緊張に対応して軍備を増強しました。",
                country.name
            ))
        }
        _ => None,
    }
}

pub(crate) fn apply_economic_drift(
    countries: &mut [CountryState],
    idx: usize,
    scale: f64,
) -> Option<String> {
    let country = &mut countries[idx];
    let drift = (country.stability - 50) as f64 * 0.4 * scale;
    if drift.abs() > 0.5 {
        country.gdp = (country.gdp + drift).max(0.0);
        if drift > 0.0 {
            return Some(format!(
                "{} は安定した統治で GDP が {:.1} 増加しました。",
                country.name, drift
            ));
        } else {
            return Some(format!(
                "{} は不安定化で GDP が {:.1} 減少しました。",
                country.name,
                drift.abs()
            ));
        }
    }
    None
}

fn clamp_metric(value: i32) -> i32 {
    value.clamp(MIN_METRIC, MAX_METRIC)
}

fn clamp_resource(value: i32) -> i32 {
    value.clamp(MIN_RESOURCES, MAX_RESOURCES)
}
