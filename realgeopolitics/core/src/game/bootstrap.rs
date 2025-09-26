use anyhow::{Result, ensure};
use rand::{SeedableRng, rngs::StdRng};

use super::{
    BASE_TICK_MINUTES, MAX_METRIC, MAX_RESOURCES, MIN_METRIC, MIN_RESOURCES, MINUTES_PER_DAY,
    country::{BudgetAllocation, CountryDefinition, CountryState},
    economy::{CreditRating, FiscalAccount, IndustryCatalog, IndustryRuntime, TaxPolicy},
    event_templates::{ScriptedEventState, load_event_templates},
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

    pub(crate) fn into_bootstrap(self) -> Result<GameBootstrap> {
        self.validate_definitions()?;
        let GameBuilder { definitions, rng } = self;

        let mut countries = initialise_countries(definitions);
        diplomacy::initialise_relations(&mut countries);

        let mut scheduler = Scheduler::new();
        register_core_tasks(&mut scheduler);
        let event_templates = register_scripted_events(&mut scheduler, countries.len())?;

        let commodity_market = CommodityMarket::new(120.0, 7.5, 0.04);
        let industry_catalog = IndustryCatalog::from_embedded().unwrap_or_default();
        let industry_runtime = IndustryRuntime::from_catalog(industry_catalog);

        Ok(GameBootstrap {
            rng,
            scheduler,
            countries,
            commodity_market,
            event_templates,
            industry_runtime,
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
    pub(crate) event_templates: Vec<ScriptedEventState>,
    pub(crate) industry_runtime: IndustryRuntime,
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

fn register_scripted_events(
    scheduler: &mut Scheduler,
    country_count: usize,
) -> Result<Vec<ScriptedEventState>> {
    let event_templates = load_event_templates(country_count)?;
    for (idx, template) in event_templates.iter().enumerate() {
        let mut task = ScheduledTask::new(
            TaskKind::ScriptedEvent(idx),
            template.initial_delay_minutes(),
        );
        task = task.with_schedule(ScheduleSpec::EveryMinutes(template.check_minutes()));
        scheduler.schedule(task);
    }
    Ok(event_templates)
}

fn clamp_metric(value: i32) -> i32 {
    value.clamp(MIN_METRIC, MAX_METRIC)
}

fn clamp_resource(value: i32) -> i32 {
    value.clamp(MIN_RESOURCES, MAX_RESOURCES)
}
