use crate::game::country::CountryState;
use crate::game::{MAX_RELATION, MIN_RELATION};

pub(crate) fn initialise_relations(countries: &mut [CountryState]) {
    for i in 0..countries.len() {
        let name_i = countries[i].name.clone();
        for j in 0..countries.len() {
            if i == j {
                continue;
            }
            let name_j = countries[j].name.clone();
            countries[i].relations.insert(name_j, 50);
        }
        countries[i].relations.remove(&name_i);
    }
}

pub(crate) fn pulse(countries: &mut [CountryState]) -> Vec<String> {
    let mut reports = Vec::new();
    let len = countries.len();
    for idx in 0..len {
        for other in (idx + 1)..len {
            let partner_name = countries[other].name.clone();
            if let Some(&relation) = countries[idx].relations.get(&partner_name) {
                let adjustment = if relation > 75 {
                    -1
                } else if relation < -60 {
                    2
                } else if relation < 30 {
                    1
                } else {
                    0
                };
                if adjustment != 0 {
                    adjust_bilateral_relation(countries, idx, other, adjustment, adjustment);
                    reports.push(format!(
                        "{} と {} の関係値を調整しました (Δ {})",
                        countries[idx].name, partner_name, adjustment
                    ));
                }
            }
        }
    }
    reports
}

pub(crate) fn adjust_bilateral_relation(
    countries: &mut [CountryState],
    idx_a: usize,
    idx_b: usize,
    delta_a: i32,
    delta_b: i32,
) {
    if idx_a == idx_b {
        panic!("同じ国同士の相互関係は調整できません");
    }

    let (a_name, b_name) = {
        let a = &countries[idx_a].name;
        let b = &countries[idx_b].name;
        (a.clone(), b.clone())
    };

    if idx_a < idx_b {
        let (left, right) = countries.split_at_mut(idx_b);
        let a = &mut left[idx_a];
        let b = &mut right[0];
        if let Some(value) = a.relations.get_mut(&b_name) {
            *value = clamp_relation(*value + delta_a);
        }
        if let Some(value) = b.relations.get_mut(&a_name) {
            *value = clamp_relation(*value + delta_b);
        }
    } else {
        let (left, right) = countries.split_at_mut(idx_a);
        let b = &mut left[idx_b];
        let a = &mut right[0];
        if let Some(value) = a.relations.get_mut(&b_name) {
            *value = clamp_relation(*value + delta_a);
        }
        if let Some(value) = b.relations.get_mut(&a_name) {
            *value = clamp_relation(*value + delta_b);
        }
    }
}

fn clamp_relation(value: i32) -> i32 {
    value.clamp(MIN_RELATION, MAX_RELATION)
}
pub(crate) fn improve_relations(countries: &mut [CountryState], idx: usize, scale: f64) {
    let delta_primary = (5.0 * scale) as i32;
    let delta_secondary = (3.0 * scale) as i32;

    for partner_idx in 0..countries.len() {
        if partner_idx == idx {
            continue;
        }
        adjust_bilateral_relation(countries, idx, partner_idx, delta_primary, delta_secondary);
    }
}

pub(crate) fn penalise_after_military(countries: &mut [CountryState], idx: usize, delta: i32) {
    if delta == 0 {
        return;
    }
    for partner_idx in 0..countries.len() {
        if partner_idx == idx {
            continue;
        }
        adjust_bilateral_relation(countries, idx, partner_idx, delta, delta / 2);
    }
}
