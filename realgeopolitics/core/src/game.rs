use std::collections::HashMap;

use anyhow::{Result, anyhow, ensure};
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use serde::{Deserialize, Serialize};

use crate::{CalendarDate, GameClock, ScheduleSpec, ScheduledTask, Scheduler, TaskKind};

const BASE_TICK_MINUTES: f64 = 60.0;
const MAX_RELATION: i32 = 100;
const MIN_RELATION: i32 = -100;
const MAX_METRIC: i32 = 100;
const MIN_METRIC: i32 = 0;
const MAX_RESOURCES: i32 = 200;
const MIN_RESOURCES: i32 = 0;

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

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct BudgetAllocation {
    pub infrastructure: f64,
    pub military: f64,
    pub welfare: f64,
    pub diplomacy: f64,
}

impl BudgetAllocation {
    pub fn new(infrastructure: f64, military: f64, welfare: f64, diplomacy: f64) -> Result<Self> {
        ensure!(infrastructure.is_finite(), "インフラ配分が不正です");
        ensure!(military.is_finite(), "軍事配分が不正です");
        ensure!(welfare.is_finite(), "福祉配分が不正です");
        ensure!(diplomacy.is_finite(), "外交配分が不正です");
        ensure!(
            infrastructure >= 0.0,
            "インフラ配分は0以上で指定してください"
        );
        ensure!(military >= 0.0, "軍事配分は0以上で指定してください");
        ensure!(welfare >= 0.0, "福祉配分は0以上で指定してください");
        ensure!(diplomacy >= 0.0, "外交配分は0以上で指定してください");
        let total = infrastructure + military + welfare + diplomacy;
        ensure!(
            total <= 1.0 + f64::EPSILON,
            "配分の合計が100%を超えています: {:.1}%",
            total * 100.0
        );
        Ok(Self {
            infrastructure,
            military,
            welfare,
            diplomacy,
        })
    }

    pub fn from_percentages(
        infrastructure: f64,
        military: f64,
        welfare: f64,
        diplomacy: f64,
    ) -> Result<Self> {
        Self::new(
            infrastructure / 100.0,
            military / 100.0,
            welfare / 100.0,
            diplomacy / 100.0,
        )
    }

    pub fn total(&self) -> f64 {
        self.infrastructure + self.military + self.welfare + self.diplomacy
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
    pub budget: f64,
    pub resources: i32,
    pub relations: HashMap<String, i32>,
    allocations: BudgetAllocation,
}

impl CountryState {
    pub fn allocations(&self) -> BudgetAllocation {
        self.allocations
    }

    fn set_allocations(&mut self, allocations: BudgetAllocation) {
        self.allocations = allocations;
    }
}

const MINUTES_PER_DAY: u64 = 24 * 60;

pub struct GameState {
    clock: GameClock,
    calendar: CalendarDate,
    day_progress_minutes: u32,
    rng: StdRng,
    scheduler: Scheduler,
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

        let default_alloc = BudgetAllocation::new(0.25, 0.25, 0.25, 0.25)?;

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
                allocations: default_alloc,
            })
            .collect();

        initialise_relations(&mut countries);

        let mut scheduler = Scheduler::new();
        scheduler.schedule(
            ScheduledTask::new(TaskKind::EconomicTick, BASE_TICK_MINUTES as u64)
                .with_schedule(ScheduleSpec::EveryMinutes(BASE_TICK_MINUTES as u64)),
        );
        scheduler.schedule(
            ScheduledTask::new(TaskKind::EventTrigger, (BASE_TICK_MINUTES * 4.0) as u64)
                .with_schedule(ScheduleSpec::EveryMinutes((BASE_TICK_MINUTES * 4.0) as u64)),
        );
        scheduler.schedule(
            ScheduledTask::new(TaskKind::PolicyResolution, MINUTES_PER_DAY)
                .with_schedule(ScheduleSpec::Daily),
        );
        scheduler.schedule(
            ScheduledTask::new(TaskKind::DiplomaticPulse, (BASE_TICK_MINUTES * 6.0) as u64)
                .with_schedule(ScheduleSpec::EveryMinutes((BASE_TICK_MINUTES * 6.0) as u64)),
        );

        Ok(Self {
            clock: GameClock::new(),
            calendar: CalendarDate::from_start(),
            day_progress_minutes: 0,
            rng,
            scheduler,
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

    pub fn simulation_minutes(&self) -> f64 {
        self.clock.total_minutes_f64()
    }

    pub fn calendar_date(&self) -> CalendarDate {
        self.calendar
    }

    pub fn countries(&self) -> &[CountryState] {
        &self.countries
    }

    #[cfg(test)]
    pub fn countries_mut(&mut self) -> &mut [CountryState] {
        &mut self.countries
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

    pub fn allocations_of(&self, idx: usize) -> Result<BudgetAllocation> {
        self.countries
            .get(idx)
            .map(|country| country.allocations())
            .ok_or_else(|| anyhow!("指定された国の番号が無効です: {}", idx + 1))
    }

    pub fn update_allocations(&mut self, idx: usize, allocations: BudgetAllocation) -> Result<()> {
        let country = self
            .countries
            .get_mut(idx)
            .ok_or_else(|| anyhow!("指定された国の番号が無効です: {}", idx + 1))?;
        country.set_allocations(allocations);
        Ok(())
    }

    pub fn tick_minutes(&mut self, minutes: f64) -> Result<Vec<String>> {
        ensure!(minutes.is_finite(), "時間が不正です");
        ensure!(minutes > 0.0, "時間は正の値で指定してください");

        let advanced_minutes = self.clock.advance_minutes(minutes);
        self.update_calendar(advanced_minutes);

        let scale = minutes / BASE_TICK_MINUTES;
        let mut reports = Vec::new();

        let ready_tasks = self.scheduler.next_ready_tasks(&self.clock);
        if ready_tasks.is_empty() {
            reports.push(format!(
                "{:.1} 分経過しましたが、スケジュールされた処理はありません。",
                minutes
            ));
        } else {
            for task in ready_tasks {
                let mut task_reports = task.execute(self, scale);
                if task_reports.is_empty() {
                    continue;
                }
                reports.append(&mut task_reports);
            }
        }

        Ok(reports)
    }

    fn process_economic_tick(&mut self, scale: f64) -> Vec<String> {
        let mut reports = Vec::new();
        for idx in 0..self.countries.len() {
            let mut country_reports = self.apply_budget_effects(idx, scale);
            if let Some(event_report) = self.trigger_random_event(idx, scale) {
                country_reports.push(event_report);
            }
            if let Some(drift_report) = self.apply_economic_drift(idx, scale) {
                country_reports.push(drift_report);
            }
            reports.extend(country_reports);
        }
        reports
    }

    fn process_event_trigger(&mut self) -> Vec<String> {
        let mut reports = Vec::new();
        for country in &mut self.countries {
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

    fn process_policy_resolution(&mut self) -> Vec<String> {
        let mut reports = Vec::new();
        for country in &mut self.countries {
            let allocation = country.allocations();
            let slack = (1.0 - allocation.total()).max(0.0);
            if slack > 0.05 {
                let reserve = country.gdp * slack * 0.01;
                country.budget += reserve;
                reports.push(format!(
                    "{} は未配分予算 {:.1} を予備費に積み増しました。",
                    country.name, reserve
                ));
            }
            if country.resources < 25 {
                country.gdp = (country.gdp - 20.0).max(0.0);
                reports.push(format!(
                    "{} は資源不足で生産が停滞しています。",
                    country.name
                ));
            }
        }
        reports
    }

    fn process_diplomatic_pulse(&mut self) -> Vec<String> {
        let mut reports = Vec::new();
        let len = self.countries.len();
        for idx in 0..len {
            for other in (idx + 1)..len {
                let partner_name = self.countries[other].name.clone();
                if let Some(&relation) = self.countries[idx].relations.get(&partner_name) {
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
                        self.adjust_bilateral_relation(idx, other, adjustment, adjustment);
                        reports.push(format!(
                            "{} と {} の関係値を調整しました (Δ {})",
                            self.countries[idx].name, partner_name, adjustment
                        ));
                    }
                }
            }
        }
        reports
    }

    fn apply_budget_effects(&mut self, idx: usize, scale: f64) -> Vec<String> {
        let mut reports = Vec::new();
        let allocation = self.countries[idx].allocations();
        let total_allocation = allocation.total();

        let revenue = {
            let country = &self.countries[idx];
            (country.gdp * 0.015 * scale).max(5.0 * scale)
        };
        {
            let country = &mut self.countries[idx];
            country.budget += revenue;
        }

        if total_allocation <= f64::EPSILON {
            return reports;
        }

        let spending_capacity = {
            let country = &self.countries[idx];
            country.budget * 0.08 * scale
        };

        let mut total_spent = 0.0;

        let infra_spend = spending_capacity * allocation.infrastructure;
        if infra_spend > 0.0 {
            total_spent += infra_spend;
            let name = self.countries[idx].name.clone();
            {
                let country = &mut self.countries[idx];
                country.gdp += infra_spend * 0.9;
                country.stability = clamp_metric(country.stability + (4.0 * scale) as i32);
                country.approval = clamp_metric(country.approval + (3.0 * scale) as i32);
                country.resources = clamp_resource(country.resources - (6.0 * scale) as i32);
            }
            reports.push(format!(
                "{} がインフラ投資を実施中です (支出 {:.1})",
                name, infra_spend
            ));
        }

        let military_spend = spending_capacity * allocation.military;
        if military_spend > 0.0 {
            total_spent += military_spend;
            let name = self.countries[idx].name.clone();
            {
                let country = &mut self.countries[idx];
                country.military = clamp_metric(country.military + (5.0 * scale) as i32);
                country.stability = clamp_metric(country.stability + (2.0 * scale) as i32);
                country.approval = clamp_metric(country.approval - (3.0 * scale) as i32);
                country.resources = clamp_resource(country.resources - (4.0 * scale) as i32);
            }
            self.adjust_relations_after_military(idx, (-2.0 * scale) as i32);
            reports.push(format!(
                "{} が軍事強化に予算を充当しました (支出 {:.1})",
                name, military_spend
            ));
        }

        let welfare_spend = spending_capacity * allocation.welfare;
        if welfare_spend > 0.0 {
            total_spent += welfare_spend;
            let name = self.countries[idx].name.clone();
            {
                let country = &mut self.countries[idx];
                country.approval = clamp_metric(country.approval + (6.0 * scale) as i32);
                country.stability = clamp_metric(country.stability + (4.0 * scale) as i32);
                country.gdp = (country.gdp - welfare_spend * 0.25).max(0.0);
            }
            reports.push(format!(
                "{} が社会福祉を拡充しました (支出 {:.1})",
                name, welfare_spend
            ));
        }

        let diplomacy_spend = spending_capacity * allocation.diplomacy;
        if diplomacy_spend > 0.0 {
            total_spent += diplomacy_spend;
            let name = self.countries[idx].name.clone();
            self.improve_relations(idx, scale);
            reports.push(format!(
                "{} が外交関係の改善に取り組んでいます (支出 {:.1})",
                name, diplomacy_spend
            ));
        }

        {
            let country = &mut self.countries[idx];
            country.budget = (country.budget - total_spent).max(0.0);
        }

        reports
    }

    fn update_calendar(&mut self, advanced_minutes: u64) {
        let mut total_days = advanced_minutes / MINUTES_PER_DAY;
        let remainder = advanced_minutes % MINUTES_PER_DAY;
        self.day_progress_minutes += remainder as u32;
        if self.day_progress_minutes as u64 >= MINUTES_PER_DAY {
            total_days += (self.day_progress_minutes as u64) / MINUTES_PER_DAY;
            self.day_progress_minutes %= MINUTES_PER_DAY as u32;
        }
        if total_days > 0 {
            self.calendar.advance_days(total_days);
        }
    }

    fn improve_relations(&mut self, idx: usize, scale: f64) {
        let delta_primary = (5.0 * scale) as i32;
        let delta_secondary = (3.0 * scale) as i32;

        for partner_idx in 0..self.countries.len() {
            if partner_idx == idx {
                continue;
            }
            self.adjust_bilateral_relation(idx, partner_idx, delta_primary, delta_secondary);
        }
    }

    fn adjust_relations_after_military(&mut self, idx: usize, delta: i32) {
        if delta == 0 {
            return;
        }
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

    fn trigger_random_event(&mut self, idx: usize, scale: f64) -> Option<String> {
        let probability = (0.25 * scale).clamp(0.0, 1.0);
        if !self.rng.gen_bool(probability as f64) {
            return None;
        }

        let country = &mut self.countries[idx];
        match self.rng.gen_range(0..3) {
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

    fn apply_economic_drift(&mut self, idx: usize, scale: f64) -> Option<String> {
        let country = &mut self.countries[idx];
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
    value.clamp(MIN_METRIC, MAX_METRIC)
}

fn clamp_relation(value: i32) -> i32 {
    value.clamp(MIN_RELATION, MAX_RELATION)
}

fn clamp_resource(value: i32) -> i32 {
    value.clamp(MIN_RESOURCES, MAX_RESOURCES)
}

impl ScheduledTask {
    pub fn execute(&self, game: &mut GameState, scale: f64) -> Vec<String> {
        match self.kind {
            TaskKind::EconomicTick => game.process_economic_tick(scale),
            TaskKind::EventTrigger => game.process_event_trigger(),
            TaskKind::PolicyResolution => game.process_policy_resolution(),
            TaskKind::DiplomaticPulse => game.process_diplomatic_pulse(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scheduler::{ONE_YEAR_MINUTES, ScheduleSpec};

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
            }
        ]"#,
        )
        .unwrap()
    }

    #[test]
    fn allocations_must_not_exceed_100_percent() {
        let result = BudgetAllocation::from_percentages(40.0, 30.0, 20.0, 15.0);
        assert!(result.is_err());
    }

    #[test]
    fn infrastructure_allocation_increases_gdp() {
        let mut game = GameState::from_definitions_with_seed(sample_definitions(), 1).unwrap();
        let alloc = BudgetAllocation::from_percentages(60.0, 10.0, 15.0, 10.0).unwrap();
        game.update_allocations(0, alloc).unwrap();
        let before_gdp = game.countries()[0].gdp;
        let reports = game.tick_minutes(120.0).unwrap();
        assert!(reports.iter().any(|r| r.contains("インフラ投資")));
        assert!(game.countries()[0].gdp > before_gdp);
    }

    #[test]
    fn diplomacy_allocation_improves_relations() {
        let mut game = GameState::from_definitions_with_seed(sample_definitions(), 2).unwrap();
        let before = game.countries()[0]
            .relations
            .get("Borealis")
            .copied()
            .unwrap();
        let alloc = BudgetAllocation::from_percentages(10.0, 10.0, 10.0, 60.0).unwrap();
        game.update_allocations(0, alloc).unwrap();
        game.tick_minutes(180.0).unwrap();
        let after = game.countries()[0]
            .relations
            .get("Borealis")
            .copied()
            .unwrap();
        assert!(after > before);
    }

    #[test]
    fn scheduler_returns_ready_tasks_by_time() {
        let mut scheduler = Scheduler::new();
        scheduler.schedule(ScheduledTask::new(TaskKind::EconomicTick, 5));
        scheduler.schedule(ScheduledTask::new(TaskKind::EventTrigger, 120));
        scheduler.schedule(ScheduledTask::new(
            TaskKind::PolicyResolution,
            ONE_YEAR_MINUTES + 120,
        ));

        let mut clock = GameClock::new();
        clock.advance_minutes(5.0);
        let ready_now = scheduler.next_ready_tasks(&clock);
        assert_eq!(ready_now.len(), 1);
        assert_eq!(ready_now[0].kind, TaskKind::EconomicTick);

        let mut later_clock = GameClock::new();
        later_clock.advance_minutes(180.0);
        let ready_later = scheduler.next_ready_tasks(&later_clock);
        assert_eq!(ready_later.len(), 1);
        assert_eq!(ready_later[0].kind, TaskKind::EventTrigger);

        let mut yearly_clock = GameClock::new();
        yearly_clock.advance_minutes((ONE_YEAR_MINUTES + 240) as f64);
        let ready_after_year = scheduler.next_ready_tasks(&yearly_clock);
        assert!(
            ready_after_year
                .iter()
                .any(|task| task.kind == TaskKind::PolicyResolution)
        );
    }

    #[test]
    fn repeating_schedule_spec_requeues_task() {
        let mut scheduler = Scheduler::new();
        scheduler.schedule(
            ScheduledTask::new(TaskKind::EconomicTick, 5)
                .with_schedule(ScheduleSpec::EveryMinutes(60)),
        );

        let mut clock = GameClock::new();
        clock.advance_minutes(5.0);
        let ready_first = scheduler.next_ready_tasks(&clock);
        assert_eq!(ready_first.len(), 1);
        assert_eq!(ready_first[0].kind, TaskKind::EconomicTick);

        let mut later_clock = GameClock::new();
        later_clock.advance_minutes(65.0);
        let ready_second = scheduler.next_ready_tasks(&later_clock);
        assert_eq!(ready_second.len(), 1);
        assert_eq!(ready_second[0].kind, TaskKind::EconomicTick);
        assert!(ready_second[0].execute_at.minutes >= 60);
    }

    #[test]
    fn scheduled_task_economic_tick_applies_budget_effects() {
        let mut game = GameState::from_definitions_with_seed(sample_definitions(), 3).unwrap();
        let alloc = BudgetAllocation::from_percentages(50.0, 20.0, 20.0, 5.0).unwrap();
        game.update_allocations(0, alloc).unwrap();
        let task = ScheduledTask::new(TaskKind::EconomicTick, 0);
        let reports = task.execute(&mut game, 1.0);
        assert!(reports.iter().any(|r| r.contains("インフラ投資")));
    }

    #[test]
    fn scheduled_task_event_trigger_penalises_low_stability() {
        let mut game = GameState::from_definitions_with_seed(sample_definitions(), 4).unwrap();
        {
            let countries = game.countries_mut();
            countries[0].stability = 30;
            countries[0].approval = 50;
        }
        let task = ScheduledTask::new(TaskKind::EventTrigger, 0);
        let reports = task.execute(&mut game, 1.0);
        assert!(reports.iter().any(|r| r.contains("国民支持が低下しました")));
        assert!(game.countries()[0].approval < 50);
    }

    #[test]
    fn scheduled_task_policy_resolution_updates_reserve_and_resources() {
        let mut game = GameState::from_definitions_with_seed(sample_definitions(), 5).unwrap();
        {
            let countries = game.countries_mut();
            countries[0].resources = 20;
        }
        let alloc = BudgetAllocation::from_percentages(40.0, 20.0, 20.0, 5.0).unwrap();
        game.update_allocations(0, alloc).unwrap();
        let before_budget = game.countries()[0].budget;
        let before_gdp = game.countries()[0].gdp;
        let task = ScheduledTask::new(TaskKind::PolicyResolution, 0);
        let reports = task.execute(&mut game, 1.0);
        assert!(reports.iter().any(|r| r.contains("予備費")));
        assert!(game.countries()[0].budget > before_budget);
        assert!(game.countries()[0].gdp < before_gdp);
    }

    #[test]
    fn scheduled_task_diplomatic_pulse_adjusts_relations() {
        let mut game = GameState::from_definitions_with_seed(sample_definitions(), 6).unwrap();
        {
            let countries = game.countries_mut();
            if let Some(rel) = countries[0].relations.get_mut("Borealis") {
                *rel = 90;
            }
            if let Some(rel) = countries[1].relations.get_mut("Asteria") {
                *rel = 90;
            }
        }
        let task = ScheduledTask::new(TaskKind::DiplomaticPulse, 0);
        let reports = task.execute(&mut game, 1.0);
        assert!(reports.iter().any(|r| r.contains("関係値")));
        let relation = game.countries()[0]
            .relations
            .get("Borealis")
            .copied()
            .unwrap();
        assert!(relation < 90);
    }
}
