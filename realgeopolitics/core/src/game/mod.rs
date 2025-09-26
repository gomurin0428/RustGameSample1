mod constants;
pub(crate) use constants::*;
mod country;
mod economy;
mod event_templates;
mod market;
mod state;
pub(crate) mod systems;

pub use country::{BudgetAllocation, CountryDefinition, CountryState};
#[allow(unused_imports)]
pub use economy::{
    DependencyKind, FiscalSnapshot, FiscalTrendPoint, IndustryCatalog, IndustryCategory,
    SectorDefinition, SectorDependency, SectorId, SectorState, TaxPolicy, TaxPolicyConfig,
};
pub use state::{GameState, TimeStatus};
