use anyhow::{Result, anyhow, ensure};
use rand::SeedableRng;
use rand::rngs::StdRng;

#[cfg(test)]
use super::economy::{ExpenseKind, RevenueKind};
use super::{
    BASE_TICK_MINUTES, MAX_METRIC, MAX_RESOURCES, MIN_METRIC, MIN_RESOURCES, MINUTES_PER_DAY,
    country::{BudgetAllocation, CountryDefinition, CountryState},
    economy::{CreditRating, FiscalAccount, TaxPolicy},
    market::CommodityMarket,
    systems::{diplomacy, events, fiscal, policy},
};
use crate::{CalendarDate, GameClock, ScheduleSpec, ScheduledTask, Scheduler, TaskKind};

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

        diplomacy::initialise_relations(&mut countries);

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
            fiscal::prepare_all_fiscal_flows(&mut self.countries, scale);
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
            let mut country_reports = fiscal::apply_budget_effects(
                &mut self.countries,
                &self.commodity_market,
                idx,
                scale,
            );
            if let Some(event_report) =
                events::trigger_random_event(&mut self.countries, &mut self.rng, idx, scale)
            {
                country_reports.push(event_report);
            }
            if let Some(drift_report) =
                events::apply_economic_drift(&mut self.countries, idx, scale)
            {
                country_reports.push(drift_report);
            }
            reports.extend(country_reports);
        }

        self.fiscal_prepared = false;
        Ok(reports)
    }

    pub(crate) fn process_economic_tick(&mut self, scale: f64) -> Vec<String> {
        let mut reports = Vec::new();
        let already_prepared = self.fiscal_prepared;
        if !already_prepared {
            fiscal::prepare_all_fiscal_flows(&mut self.countries, scale);
            self.fiscal_prepared = true;
        }
        for idx in 0..self.countries.len() {
            let mut country_reports = fiscal::apply_budget_effects(
                &mut self.countries,
                &self.commodity_market,
                idx,
                scale,
            );
            if let Some(event_report) =
                events::trigger_random_event(&mut self.countries, &mut self.rng, idx, scale)
            {
                country_reports.push(event_report);
            }
            if let Some(drift_report) =
                events::apply_economic_drift(&mut self.countries, idx, scale)
            {
                country_reports.push(drift_report);
            }
            reports.extend(country_reports);
        }
        if !already_prepared {
            self.fiscal_prepared = false;
        }
        reports
    }

    pub(crate) fn process_event_trigger(&mut self) -> Vec<String> {
        events::process_event_trigger(&mut self.countries)
    }

    pub(crate) fn process_policy_resolution(&mut self) -> Vec<String> {
        policy::resolve(&mut self.countries)
    }

    pub(crate) fn process_diplomatic_pulse(&mut self) -> Vec<String> {
        diplomacy::pulse(&mut self.countries)
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
}

fn clamp_metric(value: i32) -> i32 {
    value.clamp(MIN_METRIC, MAX_METRIC)
}

fn clamp_resource(value: i32) -> i32 {
    value.clamp(MIN_RESOURCES, MAX_RESOURCES)
}

impl ScheduledTask {
    pub fn execute(&self, game: &mut GameState, scale: f64) -> Vec<String> {
        super::systems::tasks::execute(self, game, scale)
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
        assert!(!reports.iter().any(|report| report.contains("危機")));
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
