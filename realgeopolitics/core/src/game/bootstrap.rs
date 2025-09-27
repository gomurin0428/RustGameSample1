use anyhow::{Result, ensure};
use rand::{SeedableRng, rngs::StdRng};

use super::{
    BASE_TICK_MINUTES, MAX_METRIC, MAX_RESOURCES, MIN_METRIC, MIN_RESOURCES, MINUTES_PER_DAY,
    country::{BudgetAllocation, CountryDefinition, CountryState},
    economy::{CreditRating, FiscalAccount, IndustryCatalog, IndustryRuntime, TaxPolicy},
    event_templates::ScriptedEventEngine,
    industry::IndustryEngine,
    market::CommodityMarket,
    state::GameState,
    systems::diplomacy,
};
use crate::{ScheduleSpec, ScheduledTask, Scheduler, TaskKind};

pub struct GameBuilder {
    definitions: Vec<CountryDefinition>,
    rng: StdRng,
}

impl GameBuilder {
    pub fn new(definitions: Vec<CountryDefinition>) -> Self {
        Self {
            definitions,
            rng: StdRng::from_entropy(),
        }
    }

    pub fn with_rng(mut self, rng: StdRng) -> Self {
        self.rng = rng;
        self
    }

    pub fn build(self) -> Result<GameState> {
        let bootstrap = self.into_bootstrap()?;
        Ok(GameState::new(bootstrap))
    }

    /// Convert this `GameBuilder` into a fully initialized `GameBootstrap`.
    ///
    /// Performs validation of the builder's country definitions and assembles all bootstrap
    /// components required to start a game (random number generator, scheduler with core
    /// and scripted-event tasks, initialized countries and diplomatic relations, commodity
    /// market, and industry engine).
    ///
    /// # Returns
    ///
    /// A `GameBootstrap` containing `rng`, `scheduler`, `countries`, `commodity_market`,
    /// `scripted_events`, and `industry_engine` on success; an error if validation fails.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let defs = crate::tests::sample_definitions();
    /// let bootstrap = GameBuilder::new(defs).into_bootstrap().unwrap();
    /// assert!(!bootstrap.countries.is_empty());
    /// ```
    pub(crate) fn into_bootstrap(self) -> Result<GameBootstrap> {
        self.validate_definitions()?;
        let GameBuilder { definitions, rng } = self;

        let mut countries = initialise_countries(definitions);
        diplomacy::initialise_relations(&mut countries);

        let mut scheduler = Scheduler::new();
        register_core_tasks(&mut scheduler);
        let scripted_events = register_scripted_events(&mut scheduler, countries.len())?;

        let commodity_market = CommodityMarket::new(120.0, 7.5, 0.04);
        let industry_catalog = IndustryCatalog::from_embedded().unwrap_or_default();
        let industry_runtime = IndustryRuntime::from_catalog(industry_catalog);
        let industry_engine = IndustryEngine::new(industry_runtime);

        Ok(GameBootstrap {
            rng,
            scheduler,
            countries,
            commodity_market,
            scripted_events,
            industry_engine,
        })
    }

    fn validate_definitions(&self) -> Result<()> {
        ensure!(
            !self.definitions.is_empty(),
            "国が1つも定義されていません。最低1件の国を用意してください。"
        );
        Ok(())
    }
}

pub(crate) struct GameBootstrap {
    pub(crate) rng: StdRng,
    pub(crate) scheduler: Scheduler,
    pub(crate) countries: Vec<CountryState>,
    pub(crate) commodity_market: CommodityMarket,
    pub(crate) scripted_events: ScriptedEventEngine,
    pub(crate) industry_engine: IndustryEngine,
}

fn initialise_countries(definitions: Vec<CountryDefinition>) -> Vec<CountryState> {
    let default_alloc = BudgetAllocation::default();

    definitions
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
        .collect()
}

fn register_core_tasks(scheduler: &mut Scheduler) {
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
}

/// Registers scripted-event tasks for each built-in scripted event and returns the configured engine.
///
/// For each scripted event provided by the built-in engine (created for `country_count`), a `ScriptedEvent`
/// task is scheduled on `scheduler` using the engine's initial delay and its recurring check interval.
/// Propagates any error encountered while constructing the `ScriptedEventEngine`.
///
/// # Returns
///
/// `ScriptedEventEngine` containing the scripted events that were scheduled.
///
/// # Examples
///
/// ```ignore
/// # use your_crate::{Scheduler, register_scripted_events};
/// # fn make_scheduler() -> Scheduler { Scheduler::new() }
/// let mut scheduler = make_scheduler();
/// let engine = register_scripted_events(&mut scheduler, 3).expect("engine built");
/// assert!(engine.len() > 0);
/// ```
fn register_scripted_events(
    scheduler: &mut Scheduler,
    country_count: usize,
) -> Result<ScriptedEventEngine> {
    let engine = ScriptedEventEngine::from_builtin(country_count)?;
    for idx in 0..engine.len() {
        let mut task = ScheduledTask::new(
            TaskKind::ScriptedEvent(idx),
            engine.initial_delay_minutes(idx),
        );
        task = task.with_schedule(ScheduleSpec::EveryMinutes(engine.check_minutes(idx)));
        scheduler.schedule(task);
    }
    Ok(engine)
}

/// Clamp a metric to the allowed metric range.
///
/// Returns the input value clamped to the inclusive range [MIN_METRIC, MAX_METRIC].
///
/// # Examples
///
/// ```ignore
/// let v = clamp_metric(7);
/// assert_eq!(clamp_metric(v), v);
/// ```
fn clamp_metric(value: i32) -> i32 {
    value.clamp(MIN_METRIC, MAX_METRIC)
}

fn clamp_resource(value: i32) -> i32 {
    value.clamp(MIN_RESOURCES, MAX_RESOURCES)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::GameClock;
    use rand::SeedableRng;

    fn sample_definitions() -> Vec<CountryDefinition> {
        vec![
            CountryDefinition {
                name: "Asteria".to_string(),
                government: "Republic".to_string(),
                population_millions: 50.0,
                gdp: 1500.0,
                stability: 60,
                military: 55,
                approval: 50,
                budget: 400.0,
                resources: 70,
                tax_policy: None,
            },
            CountryDefinition {
                name: "Borealis".to_string(),
                government: "Federation".to_string(),
                population_millions: 40.0,
                gdp: 1300.0,
                stability: 55,
                military: 60,
                approval: 45,
                budget: 380.0,
                resources: 65,
                tax_policy: None,
            },
        ]
    }

    #[test]
    fn build_rejects_empty_definition_list() {
        let result = GameBuilder::new(Vec::new()).build();
        let error = match result {
            Err(err) => err,
            Ok(_) => panic!("定義空でも GameBuilder::build が成功しました"),
        };
        assert!(error.to_string().contains("国が1つも定義されていません"));
    }

    #[test]
    fn into_bootstrap_populates_all_dependencies() {
        let builder = GameBuilder::new(sample_definitions()).with_rng(StdRng::seed_from_u64(7));
        let bootstrap = builder.into_bootstrap().expect("bootstrap result");

        let GameBootstrap {
            mut scheduler,
            countries,
            scripted_events,
            commodity_market,
            industry_engine,
            ..
        } = bootstrap;

        assert_eq!(countries.len(), 2);
        assert!(scripted_events.len() > 0);
        assert!(industry_engine.overview().len() > 0);
        assert!(commodity_market.price() > 0.0);
        assert!(scheduler.peek_next_minutes(0).is_some());

        let mut clock = GameClock::new();
        clock.advance_minutes(BASE_TICK_MINUTES as f64 + 0.5);
        let ready = scheduler.next_ready_tasks(&clock);
        assert!(
            ready
                .iter()
                .any(|task| matches!(task.kind, TaskKind::EconomicTick)),
            "EconomicTick should be scheduled at first tick"
        );
    }
}
