use std::collections::HashMap;

use anyhow::{Result, anyhow, bail, ensure};
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use serde::{Deserialize, Serialize};

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
    pub budget: f64,
    pub resources: i32,
    pub relations: HashMap<String, i32>,
    planned_action: Option<Action>,
}

impl CountryState {
    pub fn planned_action(&self) -> Option<&Action> {
        self.planned_action.as_ref()
    }

    pub fn relations(&self) -> &HashMap<String, i32> {
        &self.relations
    }

    fn set_planned_action(&mut self, action: Option<Action>) {
        self.planned_action = action;
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    Infrastructure,
    MilitaryDrill,
    WelfarePackage,
    Diplomacy { target: String },
}

impl Action {
    pub fn cost(&self) -> f64 {
        match self {
            Action::Infrastructure => 120.0,
            Action::MilitaryDrill => 150.0,
            Action::WelfarePackage => 100.0,
            Action::Diplomacy { .. } => 80.0,
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Action::Infrastructure => "インフラ投資",
            Action::MilitaryDrill => "軍事演習",
            Action::WelfarePackage => "社会福祉パッケージ",
            Action::Diplomacy { .. } => "外交ミッション",
        }
    }
}

pub struct GameState {
    turn: u32,
    rng: StdRng,
    countries: Vec<CountryState>,
}

impl GameState {
    pub fn from_definitions(definitions: Vec<CountryDefinition>) -> Result<Self> {
        Self::from_definitions_with_rng(definitions, StdRng::from_entropy())
    }

    pub fn from_definitions_with_rng(
        definitions: Vec<CountryDefinition>,
        rng: StdRng,
    ) -> Result<Self> {
        ensure!(
            !definitions.is_empty(),
            "国が1つも定義されていません。最低1件の国を用意してください。"
        );

        let mut countries: Vec<CountryState> = definitions
            .into_iter()
            .map(|definition| CountryState {
                name: definition.name,
                government: definition.government,
                population_millions: definition.population_millions,
                gdp: definition.gdp,
                stability: clamp_metric(definition.stability),
                military: clamp_metric(definition.military),
                approval: clamp_metric(definition.approval),
                budget: definition.budget.max(0.0),
                resources: clamp_resource(definition.resources),
                relations: HashMap::new(),
                planned_action: None,
            })
            .collect();

        initialise_relations(&mut countries);

        Ok(Self {
            turn: 0,
            rng,
            countries,
        })
    }

    #[cfg(test)]
    pub fn from_definitions_with_seed(
        definitions: Vec<CountryDefinition>,
        seed: u64,
    ) -> Result<Self> {
        Self::from_definitions_with_rng(definitions, StdRng::seed_from_u64(seed))
    }

    pub fn turn(&self) -> u32 {
        self.turn
    }

    pub fn countries(&self) -> &[CountryState] {
        &self.countries
    }

    pub fn find_country_index(&self, name_or_index: &str) -> Option<usize> {
        if let Ok(id) = name_or_index.parse::<usize>() {
            if id > 0 && id <= self.countries.len() {
                return Some(id - 1);
            }
        }

        let name_lower = name_or_index.to_ascii_lowercase();
        self.countries
            .iter()
            .position(|country| country.name.to_ascii_lowercase() == name_lower)
    }

    pub fn plan_action(&mut self, idx: usize, action: Action) -> Result<()> {
        if idx >= self.countries.len() {
            bail!("指定された国の番号が無効です: {}", idx + 1);
        }

        if self.countries[idx].planned_action().is_some() {
            bail!(
                "{} には既に行動が設定されています。先にキャンセルしてください。",
                self.countries[idx].name
            );
        }

        if self.countries[idx].budget < action.cost() {
            bail!(
                "{} の予算が不足しています。必要 {:.1} に対して現在 {:.1} です。",
                self.countries[idx].name,
                action.cost(),
                self.countries[idx].budget
            );
        }

        if let Action::Diplomacy { target } = &action {
            let lower = target.to_ascii_lowercase();
            ensure!(
                self.countries
                    .iter()
                    .any(|other| other.name.to_ascii_lowercase() == lower),
                "外交対象 {} が存在しません。",
                target
            );
            ensure!(
                self.countries[idx].name.to_ascii_lowercase() != lower,
                "自国に対する外交ミッションは設定できません。"
            );
        }

        let country = self
            .countries
            .get_mut(idx)
            .ok_or_else(|| anyhow!("指定された国の番号が無効です: {}", idx + 1))?;
        country.set_planned_action(Some(action));
        Ok(())
    }

    pub fn cancel_action(&mut self, idx: usize) -> Result<()> {
        if idx >= self.countries.len() {
            bail!("指定された国の番号が無効です: {}", idx + 1);
        }

        if self.countries[idx].planned_action().is_none() {
            bail!(
                "{} にはキャンセルする行動がありません。",
                self.countries[idx].name
            );
        }

        let country = self
            .countries
            .get_mut(idx)
            .ok_or_else(|| anyhow!("指定された国の番号が無効です: {}", idx + 1))?;
        country.set_planned_action(None);
        Ok(())
    }

    pub fn advance_turn(&mut self) -> Result<Vec<String>> {
        self.turn = self
            .turn
            .checked_add(1)
            .ok_or_else(|| anyhow!("ターン数がオーバーフローしました"))?;

        let mut reports = Vec::new();

        for country_index in 0..self.countries.len() {
            let action_report =
                if let Some(action) = self.countries[country_index].planned_action.clone() {
                    self.resolve_action(country_index, action)?
                } else {
                    format!(
                        "{} はこのターンで特別な行動を行いませんでした。",
                        self.countries[country_index].name
                    )
                };
            self.countries[country_index].set_planned_action(None);
            reports.push(action_report);

            if let Some(event_report) = self.trigger_random_event(country_index) {
                reports.push(event_report);
            }

            if let Some(drift_text) = self.apply_economic_drift(country_index) {
                reports.push(drift_text);
            }
        }

        Ok(reports)
    }

    fn resolve_action(&mut self, idx: usize, action: Action) -> Result<String> {
        if idx >= self.countries.len() {
            bail!("指定された国の番号が無効です: {}", idx + 1);
        }

        let cost = action.cost();
        if self.countries[idx].budget < cost {
            bail!(
                "{} の予算が不足しています。必要 {:.1} に対して現在 {:.1} です。",
                self.countries[idx].name,
                cost,
                self.countries[idx].budget
            );
        }
        self.countries[idx].budget -= cost;

        match action {
            Action::Infrastructure => {
                let name = self.countries[idx].name.clone();
                {
                    let country = &mut self.countries[idx];
                    country.gdp += 140.0;
                    country.stability = clamp_metric(country.stability + 4);
                    country.approval = clamp_metric(country.approval + 3);
                    country.resources = clamp_resource(country.resources - 4);
                }
                Ok(format!(
                    "{} はインフラ投資を実施し、GDP が改善しました。",
                    name
                ))
            }
            Action::MilitaryDrill => {
                let name = self.countries[idx].name.clone();
                {
                    let country = &mut self.countries[idx];
                    country.military = clamp_metric(country.military + 6);
                    country.stability = clamp_metric(country.stability + 2);
                    country.approval = clamp_metric(country.approval - 4);
                    country.resources = clamp_resource(country.resources - 6);
                }
                self.adjust_relations_after_military(idx, -3);
                Ok(format!(
                    "{} は軍事演習を実施し、軍事力が向上しました。",
                    name
                ))
            }
            Action::WelfarePackage => {
                let name = self.countries[idx].name.clone();
                {
                    let country = &mut self.countries[idx];
                    country.approval = clamp_metric(country.approval + 6);
                    country.stability = clamp_metric(country.stability + 3);
                    country.gdp = (country.gdp - 40.0).max(0.0);
                }
                Ok(format!(
                    "{} は社会福祉パッケージを実施し、国民からの支持を得ました。",
                    name
                ))
            }
            Action::Diplomacy { target } => {
                let target_index = self
                    .find_country_index(&target)
                    .ok_or_else(|| anyhow!("外交対象 {} が見つかりません。", target))?;
                ensure!(target_index != idx, "自国に外交することはできません。");
                let name = self.countries[idx].name.clone();
                self.adjust_bilateral_relation(idx, target_index, 9, 7);
                Ok(format!(
                    "{} は {} との外交ミッションで関係を改善しました。",
                    name, target
                ))
            }
        }
    }

    fn adjust_relations_after_military(&mut self, idx: usize, delta: i32) {
        for other_index in 0..self.countries.len() {
            if other_index == idx {
                continue;
            }
            self.adjust_bilateral_relation(idx, other_index, delta, delta / 2);
        }
    }

    fn adjust_bilateral_relation(
        &mut self,
        idx_a: usize,
        idx_b: usize,
        delta_a: i32,
        delta_b: i32,
    ) {
        if idx_a == idx_b {
            panic!("同じ国同士の相互関係は調整できません");
        }

        let (a_name, b_name) = {
            let a = &self.countries[idx_a].name;
            let b = &self.countries[idx_b].name;
            (a.clone(), b.clone())
        };

        if idx_a < idx_b {
            let (left, right) = self.countries.split_at_mut(idx_b);
            let a = &mut left[idx_a];
            let b = &mut right[0];
            if let Some(value) = a.relations.get_mut(&b_name) {
                *value = clamp_relation(*value + delta_a);
            }
            if let Some(value) = b.relations.get_mut(&a_name) {
                *value = clamp_relation(*value + delta_b);
            }
        } else {
            let (left, right) = self.countries.split_at_mut(idx_a);
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

    fn trigger_random_event(&mut self, idx: usize) -> Option<String> {
        if !self.rng.gen_bool(0.35) {
            return None;
        }

        let country = &mut self.countries[idx];
        match self.rng.gen_range(0..3) {
            0 => {
                country.gdp += 90.0;
                country.approval = clamp_metric(country.approval + 2);
                Some(format!(
                    "{} で技術革新が起き、GDP が伸びました。",
                    country.name
                ))
            }
            1 => {
                country.stability = clamp_metric(country.stability - 6);
                country.approval = clamp_metric(country.approval - 5);
                Some(format!(
                    "{} で抗議活動が拡大し、安定度が低下しました。",
                    country.name
                ))
            }
            2 => {
                country.resources = clamp_resource(country.resources - 8);
                country.military = clamp_metric(country.military + 3);
                Some(format!(
                    "{} は国境で緊張が高まり、軍事力を増強しました。",
                    country.name
                ))
            }
            _ => None,
        }
    }

    fn apply_economic_drift(&mut self, idx: usize) -> Option<String> {
        let country = &mut self.countries[idx];
        let revenue = (country.gdp * 0.018).max(25.0);
        country.budget += revenue;

        let drift = (country.stability - 50) as f64 * 0.6;
        if drift.abs() > 1.0 {
            country.gdp = (country.gdp + drift).max(0.0);
            if drift > 0.0 {
                return Some(format!(
                    "{} は安定した統治により GDP が {:.1} 増えました。",
                    country.name, drift
                ));
            } else {
                return Some(format!(
                    "{} は不安定化により GDP が {:.1} 減少しました。",
                    country.name,
                    drift.abs()
                ));
            }
        }

        None
    }
}

fn initialise_relations(countries: &mut [CountryState]) {
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

fn clamp_metric(value: i32) -> i32 {
    value.clamp(0, 100)
}

fn clamp_relation(value: i32) -> i32 {
    value.clamp(-100, 100)
}

fn clamp_resource(value: i32) -> i32 {
    value.clamp(0, 200)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_definitions() -> Vec<CountryDefinition> {
        serde_json::from_str::<Vec<CountryDefinition>>(
            r#"[
            {
                "name": "Asteria",
                "government": "Republic",
                "population_millions": 50.0,
                "gdp": 1500.0,
                "stability": 60,
                "military": 55,
                "approval": 50,
                "budget": 400.0,
                "resources": 70
            },
            {
                "name": "Borealis",
                "government": "Federation",
                "population_millions": 40.0,
                "gdp": 1300.0,
                "stability": 55,
                "military": 60,
                "approval": 45,
                "budget": 380.0,
                "resources": 65
            },
            {
                "name": "Caldoria",
                "government": "Monarchy",
                "population_millions": 30.0,
                "gdp": 1000.0,
                "stability": 50,
                "military": 52,
                "approval": 48,
                "budget": 300.0,
                "resources": 60
            }
        ]"#,
        )
        .unwrap()
    }

    #[test]
    fn loads_countries() {
        let game = GameState::from_definitions_with_seed(sample_definitions(), 1).unwrap();
        assert_eq!(game.countries().len(), 3);
        assert_eq!(game.countries()[0].relations.len(), 2);
    }

    #[test]
    fn infrastructure_improves_gdp_and_consumes_budget() {
        let mut game = GameState::from_definitions_with_seed(sample_definitions(), 2).unwrap();
        let before_gdp = game.countries()[0].gdp;
        let before_budget = game.countries()[0].budget;
        game.plan_action(0, Action::Infrastructure).unwrap();
        let reports = game.advance_turn().unwrap();
        assert!(reports.iter().any(|r| r.contains("インフラ投資")));
        assert!(game.countries()[0].gdp > before_gdp);
        assert!(game.countries()[0].budget < before_budget);
    }

    #[test]
    fn diplomacy_updates_relations_for_both_countries() {
        let mut game = GameState::from_definitions_with_seed(sample_definitions(), 3).unwrap();
        let before = game.countries()[0]
            .relations()
            .get("Borealis")
            .copied()
            .unwrap();
        game.plan_action(
            0,
            Action::Diplomacy {
                target: "Borealis".to_owned(),
            },
        )
        .unwrap();
        game.advance_turn().unwrap();
        let after_a_to_b = game.countries()[0]
            .relations()
            .get("Borealis")
            .copied()
            .unwrap();
        let after_b_to_a = game.countries()[1]
            .relations()
            .get("Asteria")
            .copied()
            .unwrap();
        assert!(after_a_to_b > before);
        assert!(after_b_to_a > before);
    }
}
