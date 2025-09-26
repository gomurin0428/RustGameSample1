mod bootstrap;
mod constants;
pub(crate) use constants::*;
mod country;
mod economy;
mod event_templates;
mod market;
mod state;
pub(crate) mod systems;

#[allow(unused_imports)]
pub use bootstrap::GameBuilder;
pub use country::{BudgetAllocation, CountryDefinition, CountryState};
#[allow(unused_imports)]
pub use economy::{
    DependencyKind, FiscalSnapshot, FiscalTrendPoint, IndustryCatalog, IndustryCategory,
    SectorDefinition, SectorDependency, SectorId, SectorOverview, SectorState, TaxPolicy,
    TaxPolicyConfig,
};
pub use state::{GameState, TimeStatus};
