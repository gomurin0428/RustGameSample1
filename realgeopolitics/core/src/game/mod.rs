mod constants;
pub(crate) use constants::*;
mod country;
mod economy;
mod event_templates;
mod market;
mod state;
pub(crate) mod systems;

pub use country::{BudgetAllocation, CountryDefinition, CountryState};
pub use economy::{FiscalSnapshot, FiscalTrendPoint, TaxPolicy, TaxPolicyConfig};
pub use state::{GameState, TimeStatus};
