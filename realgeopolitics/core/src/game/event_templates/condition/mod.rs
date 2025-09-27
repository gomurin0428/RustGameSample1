mod parser;

use crate::game::country::CountryState;

pub(crate) trait ConditionEvaluator {
    fn evaluate(&self, country: &CountryState) -> bool;
}

pub(crate) use parser::parse_condition;
