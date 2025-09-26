use anyhow::{Result, anyhow, ensure};
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

use super::country::{BudgetAllocation, CountryDefinition, CountryState};
use super::economy::{
    CreditRating, ExpenseKind, FiscalAccount, RevenueKind, TaxOutcome, TaxPolicy,
};
use super::market::CommodityMarket;
use crate::{CalendarDate, GameClock, ScheduleSpec, ScheduledTask, Scheduler, TaskKind};

const BASE_TICK_MINUTES: f64 = 60.0;
const MAX_RELATION: i32 = 100;
const MIN_RELATION: i32 = -100;
const MAX_METRIC: i32 = 100;
const MIN_METRIC: i32 = 0;
const MAX_RESOURCES: i32 = 200;
const MIN_RESOURCES: i32 = 0;

fn downgrade_rating(rating: CreditRating) -> CreditRating {
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

const MINUTES_PER_DAY: u64 = 24 * 60;

pub struct GameState {
    clock: GameClock,
    calendar: CalendarDate,
    day_progress_minutes: u32,
    time_multiplier: f64,
    rng: StdRng,
    scheduler: Scheduler,
    countries: Vec<CountryState>,
    commodity_market: CommodityMarket,
    fiscal_prepared: bool,
}

#[derive(Debug, Clone, Copy)]
pub struct TimeStatus {
    pub simulation_minutes: f64,
    pub calendar: CalendarDate,
    pub next_event_in_minutes: Option<u64>,
    pub time_multiplier: f64,
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

        let default_alloc = BudgetAllocation::default();

        let mut countries: Vec<CountryState> = definitions
            .into_iter()
            .map(|definition| {
                let initial_cash = definition.budget.max(0.0);
                let inferred_rating = if definition.approval >= 65 {
                    CreditRating::A
                } else if definition.stability >= 60 {
                    CreditRating::BBB
                } else {
                    CreditRating::BB
                };
                let tax_policy = definition
                    .tax_policy
                    .map(TaxPolicy::new)
                    .unwrap_or_else(TaxPolicy::default);
                CountryState::new(
                    definition.name,
                    definition.government,
                    definition.population_millions,
                    definition.gdp,
                    clamp_metric(definition.stability),
                    clamp_metric(definition.military),
                    clamp_metric(definition.approval),
                    clamp_resource(definition.resources),
                    FiscalAccount::new(initial_cash, inferred_rating),
                    tax_policy,
                    default_alloc,
                )
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

        let commodity_market = CommodityMarket::new(120.0, 7.5, 0.04);

        Ok(Self {
            clock: GameClock::new(),
            calendar: CalendarDate::from_start(),
            day_progress_minutes: 0,
            time_multiplier: 1.0,
            rng,
            scheduler,
            countries,
            commodity_market,
            fiscal_prepared: false,
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

    pub fn commodity_price(&self) -> f64 {
        self.commodity_market.price()
    }

    pub fn time_multiplier(&self) -> f64 {
        self.time_multiplier
    }

    pub fn set_time_multiplier(&mut self, multiplier: f64) -> Result<()> {
        ensure!(
            multiplier.is_finite() && multiplier > 0.0,
            "時間倍率は正の有限値で指定してください"
        );
        self.time_multiplier = multiplier.clamp(0.1, 5.0);
        Ok(())
    }

    pub fn next_event_minutes(&self) -> Option<u64> {
        let current = self.clock.total_minutes();
        self.scheduler
            .peek_next_minutes(current)
            .map(|next| next.saturating_sub(current))
    }

    pub fn time_status(&self) -> TimeStatus {
        TimeStatus {
            simulation_minutes: self.simulation_minutes(),
            calendar: self.calendar,
            next_event_in_minutes: self.next_event_minutes(),
            time_multiplier: self.time_multiplier,
        }
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

        let effective_minutes = minutes * self.time_multiplier;

        let advanced_minutes = self.clock.advance_minutes(effective_minutes);
        self.update_calendar(advanced_minutes);

        let scale = effective_minutes / BASE_TICK_MINUTES;
        let mut reports = Vec::new();

        if !self.fiscal_prepared {
            self.prepare_all_fiscal_flows(scale);
            self.fiscal_prepared = true;
        }

        if let Some(market_report) = self.commodity_market.update(&mut self.rng, scale) {
            reports.push(market_report);
        }

        let ready_tasks = self.scheduler.next_ready_tasks(&self.clock);
        if ready_tasks.is_empty() {
            reports.push(format!(
                "{:.1} 分経過しましたが、スケジュールされた処理はありません。",
                effective_minutes
            ));
        } else {
            for task in ready_tasks {
                let mut task_reports = task.execute(self, scale);
                if !task_reports.is_empty() {
                    reports.append(&mut task_reports);
                }
            }
        }

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

        self.fiscal_prepared = false;
        Ok(reports)
    }

    fn process_economic_tick(&mut self, scale: f64) -> Vec<String> {
        let mut reports = Vec::new();
        let already_prepared = self.fiscal_prepared;
        if !already_prepared {
            self.prepare_all_fiscal_flows(scale);
            self.fiscal_prepared = true;
        }
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
        if !already_prepared {
            self.fiscal_prepared = false;
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
        for idx in 0..self.countries.len() {
            let allocation = self.countries[idx].allocations();
            let gdp = self.countries[idx].gdp.max(0.0);

            if allocation.ensure_core_minimum {
                let min_debt = (self.countries[idx].fiscal.debt
                    * self.countries[idx].fiscal.interest_rate
                    / 360.0)
                    .max(40.0);
                let allocated_debt = (gdp * (allocation.debt_service / 100.0)).max(0.0);
                if allocated_debt + f64::EPSILON < min_debt {
                    let country = &mut self.countries[idx];
                    country.fiscal.add_debt(min_debt * 0.2);
                    let downgraded = downgrade_rating(country.fiscal.credit_rating);
                    country.fiscal.set_credit_rating(downgraded);
                    reports.push(format!(
                        "{} は債務返済が不足し、信用格付けが低下しました。",
                        country.name
                    ));
                }

                let admin_target = self.essential_administration_target(idx);
                let allocated_admin = (gdp * (allocation.administration / 100.0)).max(0.0);
                if allocated_admin + f64::EPSILON < admin_target {
                    let country = &mut self.countries[idx];
                    country.stability = clamp_metric(country.stability - 2);
                    reports.push(format!(
                        "{} は行政維持費が不足し、行政効率が悪化しています。",
                        country.name
                    ));
                }
            }

            {
                let country = &mut self.countries[idx];
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

            if self.countries[idx].resources < 25 {
                let country = &mut self.countries[idx];
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

    fn estimate_employment_ratio(&self, idx: usize) -> f64 {
        self.countries
            .get(idx)
            .map(|country| {
                let stability_factor = country.stability as f64 / MAX_METRIC as f64;
                let approval_factor = country.approval as f64 / MAX_METRIC as f64;
                ((stability_factor * 0.6) + (approval_factor * 0.4)).clamp(0.4, 1.2)
            })
            .unwrap_or(0.9)
    }

    fn prepare_all_fiscal_flows(&mut self, scale: f64) {
        for country in &mut self.countries {
            country.fiscal.clear_flows();
            country.fiscal.accrue_interest_hours(scale);
        }
    }

    fn essential_debt_target(&self, idx: usize) -> f64 {
        let country = &self.countries[idx];
        (country.fiscal.debt * country.fiscal.interest_rate / 24.0).clamp(50.0, 300.0)
    }

    fn essential_administration_target(&self, idx: usize) -> f64 {
        let country = &self.countries[idx];
        (country.population_millions * 2.0).max(35.0)
    }

    fn apply_budget_effects(&mut self, idx: usize, scale: f64) -> Vec<String> {
        let mut reports = Vec::new();
        let employment_ratio = self.estimate_employment_ratio(idx);
        let (gdp, resources) = {
            let country = &self.countries[idx];
            (country.gdp, country.resources)
        };
        let TaxOutcome {
            immediate,
            deferred,
        } = {
            let country = &mut self.countries[idx];
            country
                .tax_policy_mut()
                .collect(gdp, employment_ratio, scale)
        };
        if immediate > 0.0 {
            let country = &mut self.countries[idx];
            country
                .fiscal
                .record_revenue(RevenueKind::Taxation, immediate);
            reports.push(format!(
                "{} は税収を確保しました (即時 {:.1})",
                country.name, immediate
            ));
        }
        if deferred > 0.0 {
            reports.push(format!(
                "{} は将来計上予定の税収 {:.1} を繰越します。",
                self.countries[idx].name, deferred
            ));
        }

        let allocation = self.countries[idx].allocations();
        let gdp_amount = gdp.max(0.0);
        let percent_to_amount = |percent: f64| -> f64 {
            if percent <= 0.0 || gdp_amount <= 0.0 {
                0.0
            } else {
                gdp_amount * (percent / 100.0)
            }
        };

        let resource_revenue = self.commodity_market.revenue_for(resources, scale);
        if resource_revenue > 0.0 {
            let price_snapshot = self.commodity_market.price();
            let country = &mut self.countries[idx];
            country
                .fiscal
                .record_revenue(RevenueKind::ResourceExport, resource_revenue);
            reports.push(format!(
                "{} は資源輸出で {:.1} の外貨収入を獲得しました (単価 {:.1})",
                country.name, resource_revenue, price_snapshot
            ));
        }

        let debt_base = percent_to_amount(allocation.debt_service);
        let debt_request = if allocation.ensure_core_minimum {
            debt_base.max(self.essential_debt_target(idx))
        } else {
            debt_base
        };
        let debt_desired = debt_request * scale;
        if debt_desired > 0.0 {
            let available = self.countries[idx].fiscal.cash_reserve();
            let actual = debt_desired.min(available);
            if actual > 0.0 {
                let country = &mut self.countries[idx];
                country
                    .fiscal
                    .record_expense(ExpenseKind::DebtService, actual);
                let reduction = actual.min(country.fiscal.debt);
                if reduction > 0.0 {
                    country.fiscal.add_debt(-reduction);
                }
                reports.push(format!(
                    "{} は債務返済に {:.1} を充当しました。",
                    country.name, actual
                ));
            } else if allocation.ensure_core_minimum {
                let country = &mut self.countries[idx];
                country.fiscal.add_debt(debt_desired * 0.25);
                reports.push(format!(
                    "{} は債務返済資金が不足し、返済を繰り延べました。",
                    country.name
                ));
            }
        }

        let administration_base = percent_to_amount(allocation.administration);
        let administration_request = if allocation.ensure_core_minimum {
            administration_base.max(self.essential_administration_target(idx))
        } else {
            administration_base
        };
        let administration_desired = administration_request * scale;
        if administration_desired > 0.0 {
            let available = self.countries[idx].fiscal.cash_reserve();
            let actual = administration_desired.min(available);
            if actual > 0.0 {
                let country = &mut self.countries[idx];
                country
                    .fiscal
                    .record_expense(ExpenseKind::Administration, actual);
                let stability_gain = (actual / 120.0).round() as i32;
                country.stability = clamp_metric(country.stability + stability_gain);
                reports.push(format!(
                    "{} は行政維持に {:.1} を投じています。",
                    country.name, actual
                ));
            } else if allocation.ensure_core_minimum {
                let country = &mut self.countries[idx];
                country.stability = clamp_metric(country.stability - 3);
                reports.push(format!(
                    "{} は行政費の不足で行政効率が低下しています。",
                    country.name
                ));
            }
        }

        let infra_desired = percent_to_amount(allocation.infrastructure) * scale;
        if infra_desired > 0.0 {
            let available = self.countries[idx].fiscal.cash_reserve();
            let actual = infra_desired.min(available);
            if actual > 0.0 {
                let country = &mut self.countries[idx];
                country
                    .fiscal
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
            let available = self.countries[idx].fiscal.cash_reserve();
            let actual = welfare_desired.min(available);
            if actual > 0.0 {
                let country = &mut self.countries[idx];
                country.fiscal.record_expense(ExpenseKind::Welfare, actual);
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
            let available = self.countries[idx].fiscal.cash_reserve();
            let actual = research_desired.min(available);
            if actual > 0.0 {
                let country = &mut self.countries[idx];
                country.fiscal.record_expense(ExpenseKind::Research, actual);
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
            let available = self.countries[idx].fiscal.cash_reserve();
            let actual = diplomacy_desired.min(available);
            if actual > 0.0 {
                let country_name = self.countries[idx].name.clone();
                {
                    let country = &mut self.countries[idx];
                    country
                        .fiscal
                        .record_expense(ExpenseKind::Diplomacy, actual);
                }
                let relation_scale = (actual / 120.0).max(scale);
                self.improve_relations(idx, relation_scale);
                reports.push(format!(
                    "{} が外交関係の改善に取り組んでいます (支出 {:.1})",
                    country_name, actual
                ));
            }
        }

        let military_desired = percent_to_amount(allocation.military) * scale;
        if military_desired > 0.0 {
            let available = self.countries[idx].fiscal.cash_reserve();
            let actual = military_desired.min(available);
            if actual > 0.0 {
                let country_name = self.countries[idx].name.clone();
                {
                    let country = &mut self.countries[idx];
                    country.fiscal.record_expense(ExpenseKind::Military, actual);
                    let intensity = (actual / 80.0).round() as i32;
                    country.military = clamp_metric(country.military + intensity);
                    country.stability = clamp_metric(country.stability + (intensity / 2));
                    country.approval = clamp_metric(country.approval - (intensity / 2));
                    country.resources = clamp_resource(country.resources - (actual / 40.0) as i32);
                }
                let relation_penalty = -((2.0 * scale.max(1.0)).round() as i32);
                self.adjust_relations_after_military(idx, relation_penalty);
                reports.push(format!(
                    "{} が軍事強化に予算を充当しました (支出 {:.1})",
                    country_name, actual
                ));
            }
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
    fn allocations_reject_negative_values() {
        let result = BudgetAllocation::new(-5.0, 3.0, 4.0, 2.0, 1.0, 1.0, 1.0, true);
        assert!(result.is_err());
    }

    #[test]
    fn core_minimum_penalises_underfunding() {
        let mut game = GameState::from_definitions_with_seed(sample_definitions(), 13).unwrap();
        {
            let country = &mut game.countries_mut()[0];
            country.fiscal_mut().add_debt(400.0);
        }
        let baseline_rating = game.countries()[0].fiscal.credit_rating;
        let baseline_stability = game.countries()[0].stability;
        let allocation = BudgetAllocation::new(4.5, 3.0, 3.5, 2.0, 1.0, 1.2, 1.0, true).unwrap();
        game.update_allocations(0, allocation).unwrap();
        let task = ScheduledTask::new(TaskKind::PolicyResolution, 0);
        let reports = task.execute(&mut game, 1.0);
        assert!(
            reports
                .iter()
                .any(|report| report.contains("信用格付けが低下しました"))
        );
        let country = &game.countries()[0];
        assert_ne!(country.fiscal.credit_rating, baseline_rating);
        assert!(country.fiscal.debt > 400.0);
        assert!(country.stability < baseline_stability);
    }

    #[test]
    fn disabling_core_minimum_avoids_penalty() {
        let mut game = GameState::from_definitions_with_seed(sample_definitions(), 14).unwrap();
        {
            let country = &mut game.countries_mut()[0];
            country.fiscal_mut().add_debt(400.0);
        }
        let baseline_rating = game.countries()[0].fiscal.credit_rating;
        let baseline_stability = game.countries()[0].stability;
        let allocation = BudgetAllocation::new(4.5, 3.0, 3.5, 2.0, 1.0, 1.2, 1.0, false).unwrap();
        game.update_allocations(0, allocation).unwrap();
        let task = ScheduledTask::new(TaskKind::PolicyResolution, 0);
        let reports = task.execute(&mut game, 1.0);
        assert!(
            !reports
                .iter()
                .any(|report| report.contains("信用格付けが低下しました"))
        );
        let country = &game.countries()[0];
        assert_eq!(country.fiscal.credit_rating, baseline_rating);
        assert_eq!(country.stability, baseline_stability);
        assert!(country.fiscal.debt >= 400.0);
    }

    #[test]
    fn infrastructure_allocation_increases_gdp() {
        let mut game = GameState::from_definitions_with_seed(sample_definitions(), 1).unwrap();
        let alloc = BudgetAllocation::new(15.0, 5.0, 6.0, 3.0, 5.0, 3.0, 4.0, true).unwrap();
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
        let alloc = BudgetAllocation::new(5.0, 4.0, 4.0, 18.0, 4.0, 3.0, 3.0, true).unwrap();
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
        let alloc = BudgetAllocation::new(14.0, 8.0, 6.0, 4.0, 6.0, 3.5, 5.0, true).unwrap();
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
        let alloc = BudgetAllocation::new(10.0, 6.0, 6.0, 4.0, 6.0, 5.0, 4.0, true).unwrap();
        game.update_allocations(0, alloc).unwrap();
        let before_cash = game.countries()[0].cash_reserve();
        let before_gdp = game.countries()[0].gdp;
        let task = ScheduledTask::new(TaskKind::PolicyResolution, 0);
        let reports = task.execute(&mut game, 1.0);
        assert!(reports.iter().any(|r| r.contains("予備費")));
        assert!(game.countries()[0].cash_reserve() > before_cash);
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

    #[test]
    fn time_multiplier_scales_tick_minutes() {
        let mut game = GameState::from_definitions_with_seed(sample_definitions(), 7).unwrap();
        game.set_time_multiplier(2.0).unwrap();
        game.tick_minutes(60.0).unwrap();
        assert!(game.simulation_minutes() >= 119.0);
    }

    #[test]
    fn time_status_reflects_clock_and_next_event() {
        let mut game = GameState::from_definitions_with_seed(sample_definitions(), 8).unwrap();
        game.set_time_multiplier(2.5).unwrap();
        game.tick_minutes(30.0).unwrap();
        let status = game.time_status();
        assert!(status.simulation_minutes >= 74.0);
        assert!(status.next_event_in_minutes.is_some());
        assert_eq!(status.time_multiplier, 2.5);
        assert_eq!(status.calendar.day, 1);
    }

    #[test]
    fn fiscal_account_records_flows() {
        let mut account = FiscalAccount::new(100.0, CreditRating::BBB);
        account.record_revenue(RevenueKind::Taxation, 50.0);
        account.record_expense(ExpenseKind::Infrastructure, 20.0);
        assert!((account.cash_reserve() - 130.0).abs() < f64::EPSILON);
        assert_eq!(account.total_revenue(), 50.0);
        assert_eq!(account.total_expense(), 20.0);
        assert!((account.net_cash_flow() - 30.0).abs() < f64::EPSILON);
    }

    #[test]
    fn fiscal_snapshot_updates_after_tick() {
        let mut game = GameState::from_definitions_with_seed(sample_definitions(), 9).unwrap();
        let before_cash = game.countries()[0].cash_reserve();
        game.tick_minutes(60.0).unwrap();
        let country = &game.countries()[0];
        assert!(country.total_revenue() > 0.0);
        assert!(country.total_expense() >= 0.0);
        let net_change = country.cash_reserve() - before_cash;
        assert!((net_change - country.net_cash_flow()).abs() < 1e-6);
    }

    #[test]
    fn tax_policy_deferred_revenue_accrues() {
        let mut game = GameState::from_definitions_with_seed(sample_definitions(), 10).unwrap();
        let initial_pending = game.countries()[0].tax_policy().pending_revenue();
        assert!(initial_pending.abs() < f64::EPSILON);
        game.tick_minutes(60.0).unwrap();
        let (pending_after_first, cash_after_first) = {
            let first_country = &game.countries()[0];
            assert!(first_country.total_revenue() > 0.0);
            (
                first_country.tax_policy().pending_revenue(),
                first_country.cash_reserve(),
            )
        };
        assert!(pending_after_first > 0.0);
        game.tick_minutes(60.0).unwrap();
        let second_country = &game.countries()[0];
        assert!(second_country.total_revenue() > 0.0);
        assert!(second_country.cash_reserve() >= cash_after_first);
        assert!(second_country.tax_policy().pending_revenue() >= 0.0);
    }

    #[test]
    fn commodity_market_generates_resource_revenue() {
        let mut game = GameState::from_definitions_with_seed(sample_definitions(), 11).unwrap();
        {
            let countries = game.countries_mut();
            let policy = countries[0].tax_policy_mut();
            policy.income_rate = 0.0;
            policy.corporate_rate = 0.0;
            policy.consumption_rate = 0.0;
        }
        let before_cash = game.countries()[0].cash_reserve();
        let initial_price = game.commodity_price();
        game.tick_minutes(60.0).unwrap();
        let country = &game.countries()[0];
        assert!(country.cash_reserve() > before_cash);
        assert!(country.total_revenue() > 0.0);
        let updated_price = game.commodity_price();
        assert!(updated_price > 0.0);
        assert!((updated_price - initial_price).abs() > 0.01 || updated_price != initial_price);
    }

    #[test]
    fn next_event_minutes_reports_difference() {
        let game = GameState::from_definitions_with_seed(sample_definitions(), 12).unwrap();
        let next = game.next_event_minutes().unwrap();
        assert!(next > 0);
    }
}
