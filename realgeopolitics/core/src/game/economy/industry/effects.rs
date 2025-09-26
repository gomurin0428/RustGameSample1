use std::collections::HashMap;
use std::str::FromStr;

use anyhow::{Result, bail, ensure};

use super::model::{
    DependencyKind, IndustryCatalog, IndustryCategory, SectorDefinition, SectorId, SectorMetrics,
    SectorModifier, SectorState,
};

pub(crate) fn resolve_sector_token(catalog: &IndustryCatalog, token: &str) -> Result<SectorId> {
    let raw = token.trim();
    ensure!(!raw.is_empty(), "セクターを指定してください。");
    let mut splits = raw.split(|c| c == ':' || c == '/');
    let first = splits.next().expect("split は少なくとも1要素");
    if let Some(second) = splits.next() {
        let category = IndustryCategory::from_str(first)?;
        let key = second.trim();
        ensure!(!key.is_empty(), "セクターキーが空です。");
        let id = SectorId::new(category, key);
        ensure!(
            catalog.get(&id).is_some(),
            "指定されたセクターは存在しません: {}",
            raw
        );
        return Ok(id);
    }

    let mut matches = catalog
        .sectors()
        .filter(|(id, _)| id.key.eq_ignore_ascii_case(raw))
        .map(|(id, _)| id.clone())
        .collect::<Vec<_>>();
    matches.sort_by(|a, b| a.category.cmp(&b.category));
    match matches.len() {
        0 => bail!("セクターが見つかりません: {}", raw),
        1 => Ok(matches.remove(0)),
        _ => bail!(
            "セクター名が複数カテゴリに存在します: {} (category:key 形式で指定してください)",
            raw
        ),
    }
}

pub(crate) fn apply_subsidy(
    catalog: &IndustryCatalog,
    modifiers: &mut HashMap<SectorId, SectorModifier>,
    states: &mut HashMap<SectorId, SectorState>,
    id: &SectorId,
    percent: f64,
) -> Result<()> {
    ensure!(catalog.get(id).is_some(), "セクターが存在しません。");
    ensure!(
        percent.is_finite(),
        "補助率は有限の数値で指定してください。"
    );
    ensure!(percent >= 0.0, "補助率は0%以上で指定してください。");
    let ratio = (percent / 100.0).clamp(0.0, 0.9);
    let modifier = modifiers.entry(id.clone()).or_default();
    modifier.subsidy_bonus = ratio;
    modifier.remaining_minutes = f64::MAX;
    if let Some(state) = states.get_mut(id) {
        state.subsidy_rate = ratio;
    }
    Ok(())
}

pub(crate) fn compute_dependency_effects(
    category: IndustryCategory,
    def: &SectorDefinition,
    metrics: &HashMap<SectorId, SectorMetrics>,
) -> (f64, f64, f64) {
    let mut input_factor = 1.0;
    let mut cost_factor = 1.0;
    let mut demand_signal = 0.0;
    for dep in &def.dependencies {
        let dep_id = dep.resolve_sector(category);
        let supply_ratio = metrics
            .get(&dep_id)
            .map(|m| {
                let requirement = dep.requirement.max(0.01);
                (m.output / (def.base_output * requirement)).clamp(0.0, 2.0)
            })
            .unwrap_or(0.0);
        match dep.dependency {
            DependencyKind::Input => {
                let shortage = (0.8 - supply_ratio).max(0.0);
                let surplus = (supply_ratio - 1.2).max(0.0);
                input_factor *= (1.0 - shortage).clamp(0.0, 1.0);
                if surplus > 0.0 {
                    input_factor *= 1.0 + (surplus.min(0.5) * 0.05);
                }
            }
            DependencyKind::Cost => {
                let adjustment = 1.0 - dep.elasticity * (supply_ratio - 1.0);
                cost_factor *= adjustment.clamp(0.5, 1.5);
            }
            DependencyKind::Demand => {
                demand_signal += dep.elasticity * (supply_ratio - 1.0);
            }
        }
    }
    (input_factor.max(0.0), cost_factor.max(0.1), demand_signal)
}

pub(crate) fn sigmoid_price(signal: f64, sensitivity: f64) -> f64 {
    let logistic = 1.0 / (1.0 + (-3.0 * signal).exp());
    let centered = (logistic - 0.5) * 2.0;
    (1.0 + sensitivity * centered).clamp(0.2, 2.5)
}

pub(crate) fn update_energy_cost_index(baseline_output: f64, energy_output_total: f64) -> f64 {
    if energy_output_total <= f64::EPSILON {
        1.5
    } else {
        (baseline_output / energy_output_total).clamp(0.5, 1.6)
    }
}
