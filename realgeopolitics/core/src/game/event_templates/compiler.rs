use std::fmt;

use anyhow::{Result, anyhow};
use serde::Deserialize;

use crate::game::country::CountryState;
use crate::game::{MAX_METRIC, MAX_RESOURCES, MIN_METRIC, MIN_RESOURCES};

use super::condition::{ConditionEvaluator, parse_condition};

pub(super) fn compile_template(
    source_index: usize,
    raw: EventTemplateRaw,
) -> Result<CompiledEventTemplate> {
    CompiledEventTemplate::new(raw).map_err(|err| {
        anyhow!(
            "イベントテンプレート {} のコンパイルに失敗しました: {}",
            source_index,
            err
        )
    })
}

#[derive(Debug, Deserialize)]
pub(super) struct EventTemplateRaw {
    id: String,
    name: String,
    description: String,
    condition: String,
    #[serde(default = "EventTemplateRaw::default_check_minutes")]
    check_minutes: u64,
    #[serde(default)]
    initial_delay_minutes: u64,
    #[serde(default = "EventTemplateRaw::default_cooldown_minutes")]
    cooldown_minutes: u64,
    #[serde(default)]
    effects: Vec<EventEffectRaw>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
pub(super) enum EventEffectRaw {
    #[serde(rename = "adjust_metric")]
    AdjustMetric { metric: String, delta: f64 },
    #[serde(rename = "report")]
    Report { message: String },
}

impl EventTemplateRaw {
    const fn default_check_minutes() -> u64 {
        120
    }

    const fn default_cooldown_minutes() -> u64 {
        720
    }
}
pub(super) struct CompiledEventTemplate {
    id: String,
    name: String,
    description: String,
    check_minutes: u64,
    initial_delay_minutes: u64,
    cooldown_minutes: f64,
    condition: Box<dyn ConditionEvaluator>,
    effects: Vec<CompiledEffect>,
}

impl fmt::Debug for CompiledEventTemplate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CompiledEventTemplate")
            .field("id", &self.id)
            .field("name", &self.name)
            .field("description", &self.description)
            .field("check_minutes", &self.check_minutes)
            .field("initial_delay_minutes", &self.initial_delay_minutes)
            .field("cooldown_minutes", &self.cooldown_minutes)
            .field("effects", &self.effects)
            .finish()
    }
}

impl CompiledEventTemplate {
    fn new(raw: EventTemplateRaw) -> Result<Self> {
        if raw.check_minutes == 0 {
            return Err(anyhow!("check_minutes は 1 以上である必要があります"));
        }
        let condition = parse_condition(&raw.condition)?;
        let mut effects = Vec::with_capacity(raw.effects.len());
        for effect in raw.effects {
            effects.push(CompiledEffect::from_raw(effect)?);
        }
        Ok(Self {
            id: raw.id,
            name: raw.name,
            description: raw.description,
            check_minutes: raw.check_minutes,
            initial_delay_minutes: raw.initial_delay_minutes,
            cooldown_minutes: raw.cooldown_minutes as f64,
            condition,
            effects,
        })
    }

    pub(super) fn id(&self) -> &str {
        &self.id
    }

    pub(super) fn name(&self) -> &str {
        &self.name
    }

    pub(super) fn description(&self) -> &str {
        &self.description
    }

    pub(super) fn check_minutes(&self) -> u64 {
        self.check_minutes
    }

    pub(super) fn initial_delay_minutes(&self) -> u64 {
        self.initial_delay_minutes
    }

    pub(super) fn can_trigger(
        &self,
        country: &CountryState,
        last_triggered_at: Option<f64>,
        current_minutes: f64,
    ) -> bool {
        if !self.condition_matches(country) {
            return false;
        }
        if let Some(last) = last_triggered_at {
            if current_minutes - last < self.cooldown_minutes {
                return false;
            }
        }
        true
    }

    fn condition_matches(&self, country: &CountryState) -> bool {
        self.condition.evaluate(country)
    }
}
#[derive(Debug, Clone)]
enum CompiledEffect {
    AdjustMetric { metric: MetricField, delta: f64 },
    Report { message: String },
}

impl CompiledEffect {
    fn from_raw(raw: EventEffectRaw) -> Result<Self> {
        match raw {
            EventEffectRaw::AdjustMetric { metric, delta } => {
                let field = MetricField::from_str(&metric)?;
                Ok(Self::AdjustMetric {
                    metric: field,
                    delta,
                })
            }
            EventEffectRaw::Report { message } => Ok(Self::Report { message }),
        }
    }
}
#[derive(Debug, Clone, Copy)]
enum MetricField {
    Stability,
    Approval,
    Military,
    Resources,
    Gdp,
    Debt,
    CashReserve,
}

impl MetricField {
    fn from_str(value: &str) -> Result<Self> {
        match value {
            "stability" => Ok(Self::Stability),
            "approval" => Ok(Self::Approval),
            "military" => Ok(Self::Military),
            "resources" => Ok(Self::Resources),
            "gdp" => Ok(Self::Gdp),
            "debt" => Ok(Self::Debt),
            "cash_reserve" => Ok(Self::CashReserve),
            other => Err(anyhow!("未知のメトリクス '{}' が指定されました", other)),
        }
    }

    fn apply(&self, country: &mut CountryState, delta: f64) {
        match self {
            MetricField::Stability => {
                country.stability = clamp_metric_delta(country.stability, delta);
            }
            MetricField::Approval => {
                country.approval = clamp_metric_delta(country.approval, delta);
            }
            MetricField::Military => {
                country.military = clamp_metric_delta(country.military, delta);
            }
            MetricField::Resources => {
                country.resources = clamp_resource_delta(country.resources, delta);
            }
            MetricField::Gdp => {
                country.gdp = (country.gdp + delta).max(0.0);
            }
            MetricField::Debt => {
                country.fiscal_mut().add_debt(delta);
            }
            MetricField::CashReserve => {
                let updated = (country.fiscal.cash_reserve() + delta).max(0.0);
                country.fiscal_mut().set_cash_reserve(updated);
            }
        }
    }
}
fn clamp_metric_delta(base: i32, delta: f64) -> i32 {
    let candidate = (base as f64 + delta).round() as i32;
    candidate.clamp(MIN_METRIC, MAX_METRIC)
}

fn clamp_resource_delta(base: i32, delta: f64) -> i32 {
    let candidate = (base as f64 + delta).round() as i32;
    candidate.clamp(MIN_RESOURCES, MAX_RESOURCES)
}
impl CompiledEventTemplate {
    pub(super) fn apply_effects(&self, country: &mut CountryState) -> Vec<String> {
        let mut reports = Vec::new();
        for effect in &self.effects {
            match effect {
                CompiledEffect::AdjustMetric { metric, delta } => {
                    metric.apply(country, *delta);
                }
                CompiledEffect::Report { message } => {
                    reports.push(format_message(message, country));
                }
            }
        }
        reports
    }
}
fn format_message(template: &str, country: &CountryState) -> String {
    template.replace("{country}", &country.name)
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::country::{BudgetAllocation, CountryState};
    use crate::game::economy::CreditRating;
    use crate::game::economy::{FiscalAccount, TaxPolicy};

    fn sample_country() -> CountryState {
        CountryState::new(
            "Testland".to_string(),
            "Republic".to_string(),
            10.0,
            500.0,
            50,
            40,
            45,
            60,
            FiscalAccount::new(200.0, CreditRating::A),
            TaxPolicy::default(),
            BudgetAllocation::default(),
        )
    }

    #[test]
    fn compile_template_rejects_zero_check_minutes() {
        let raw = EventTemplateRaw {
            id: "invalid".to_string(),
            name: "Invalid".to_string(),
            description: "desc".to_string(),
            condition: "approval > 0".to_string(),
            check_minutes: 0,
            initial_delay_minutes: 0,
            cooldown_minutes: 60,
            effects: Vec::new(),
        };
        let err = compile_template(3, raw).expect_err("check_minutes == 0 should be rejected");
        assert!(err.to_string().contains("check_minutes"));
    }

    #[test]
    fn compiled_template_exposes_metadata_and_effects() {
        let raw = EventTemplateRaw {
            id: "approval_push".to_string(),
            name: "Approval Push".to_string(),
            description: "desc".to_string(),
            condition: "approval >= 40".to_string(),
            check_minutes: 60,
            initial_delay_minutes: 5,
            cooldown_minutes: 120,
            effects: vec![
                EventEffectRaw::AdjustMetric {
                    metric: "approval".to_string(),
                    delta: 10.0,
                },
                EventEffectRaw::Report {
                    message: "{country} improved approval".to_string(),
                },
            ],
        };
        let template = compile_template(0, raw).expect("valid template should compile");
        assert_eq!(template.check_minutes(), 60);
        assert_eq!(template.initial_delay_minutes(), 5);
        assert_eq!(template.id(), "approval_push");

        let mut country = sample_country();
        assert!(template.can_trigger(&country, None, 300.0));
        let reports = template.apply_effects(&mut country);
        assert_eq!(reports.len(), 1);
        assert_eq!(reports[0], "Testland improved approval");
        assert_eq!(country.approval, 55);

        assert!(
            !template.can_trigger(&country, Some(300.0), 360.0),
            "cooldown should prevent immediate re-trigger"
        );
    }
}
