use std::fmt;

use anyhow::{Result, anyhow};
use serde::Deserialize;

use crate::game::country::CountryState;
use crate::game::{MAX_METRIC, MAX_RESOURCES, MIN_METRIC, MIN_RESOURCES};

use super::condition::{ConditionEvaluator, parse_condition};

/// Compile a raw event template into a validated CompiledEventTemplate.
///
/// This validates and converts `EventTemplateRaw` into a `CompiledEventTemplate`.
/// If compilation fails, the returned error is annotated with the `source_index` to
/// indicate which input template caused the failure.
///
/// # Examples
///
/// ```
/// # use anyhow::Result;
/// # use crate::{compile_template, EventTemplateRaw, EventEffectRaw};
/// let raw = EventTemplateRaw {
///     id: "evt1".into(),
///     name: "Test Event".into(),
///     description: "A test".into(),
///     condition: "true".into(),
///     check_minutes: 120,
///     initial_delay_minutes: 0,
///     cooldown_minutes: 720,
///     effects: vec![],
/// };
/// let compiled = compile_template(0, raw).unwrap();
/// assert_eq!(compiled.check_minutes(), 120);
/// ```
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

    /// Default cooldown interval for event templates, in minutes.
    
    ///
    
    /// This value is used when a template does not specify a custom cooldown period.
    
    ///
    
    /// # Returns
    
    ///
    
    /// The default cooldown interval in minutes: `720`.
    
    ///
    
    /// # Examples
    
    ///
    
    /// ```
    
    /// assert_eq!(default_cooldown_minutes(), 720);
    
    /// ```
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
    /// Compiles an EventTemplateRaw into a CompiledEventTemplate, validating required fields and parsing the condition.
    ///
    /// Validates that `check_minutes` is at least 1, parses the `condition` expression, and converts each raw effect
    /// into its compiled representation.
    ///
    /// # Returns
    ///
    /// `Ok(CompiledEventTemplate)` when the raw template is valid and all effects and the condition were compiled successfully;
    /// `Err` when validation fails or parsing/conversion of the condition or effects fails.
    ///
    /// # Examples
    ///
    /// ```
    /// let raw = EventTemplateRaw {
    ///     id: "evt_1".into(),
    ///     name: "Test Event".into(),
    ///     description: "A simple test".into(),
    ///     condition: "approval >= 0".into(),
    ///     check_minutes: 1,
    ///     initial_delay_minutes: 0,
    ///     cooldown_minutes: 10,
    ///     effects: vec![],
    /// };
    /// let compiled = CompiledEventTemplate::new(raw).unwrap();
    /// assert_eq!(compiled.check_minutes(), 1);
    /// ```
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

    /// The template's identifier.
    ///
    /// # Examples
    ///
    /// ```
    /// let tmpl = CompiledEventTemplate {
    ///     id: "evt_1".to_string(),
    ///     name: "Test".to_string(),
    ///     description: "".to_string(),
    ///     check_minutes: 60,
    ///     initial_delay_minutes: 0,
    ///     cooldown_minutes: 120.0,
    ///     condition: Box::new(|_| true), // placeholder; actual type implements ConditionEvaluator
    ///     effects: vec![],
    /// };
    /// assert_eq!(tmpl.id(), "evt_1");
    /// ```
    ///
    /// # Returns
    ///
    /// `&str` with the template's identifier.
    pub(super) fn id(&self) -> &str {
        &self.id
    }

    /// The template's human-readable name.
    ///
    /// # Examples
    ///
    /// ```
    /// // Given a `CompiledEventTemplate` instance `template`:
    /// let name = template.name();
    /// println!("{}", name);
    /// ```
    pub(super) fn name(&self) -> &str {
        &self.name
    }

    /// Gets the event template's description.
    ///
    /// # Examples
    ///
    /// ```
    /// let tmpl = CompiledEventTemplate {
    ///     id: "e".into(),
    ///     name: "n".into(),
    ///     description: "Test description".into(),
    ///     check_minutes: 60,
    ///     initial_delay_minutes: 0,
    ///     cooldown_minutes: 120.0,
    ///     condition: Box::new(crate::condition::AlwaysTrue),
    ///     effects: vec![],
    /// };
    /// assert_eq!(tmpl.description(), "Test description");
    /// ```
    pub(super) fn description(&self) -> &str {
        &self.description
    }

    /// Configured interval, in minutes, between automatic checks for this template.
    ///
    /// # Returns
    ///
    /// The check interval in minutes.
    ///
    /// # Examples
    ///
    /// ```
    /// // assuming `tmpl` is a `CompiledEventTemplate`
    /// let interval = tmpl.check_minutes();
    /// assert!(interval > 0);
    /// ```
    pub(super) fn check_minutes(&self) -> u64 {
        self.check_minutes
    }

    /// Initial delay in minutes before the event first becomes eligible to trigger.
    ///
    /// # Returns
    ///
    /// `u64` containing the number of minutes of initial delay.
    ///
    /// # Examples
    ///
    /// ```
    /// // Given a compiled event template `template`, get its initial delay:
    /// let delay = template.initial_delay_minutes();
    /// assert!(delay >= 0);
    /// ```
    pub(super) fn initial_delay_minutes(&self) -> u64 {
        self.initial_delay_minutes
    }

    /// Determines whether this event template may trigger for `country` at `current_minutes`.
    ///
    /// The template may trigger only if its compiled condition matches the given country and, if
    /// `last_triggered_at` is provided, at least `cooldown_minutes` have elapsed since that time.
    ///
    /// # Parameters
    ///
    /// - `last_triggered_at`: previous trigger time in minutes, or `None` if never triggered.
    /// - `current_minutes`: current time in minutes.
    ///
    /// # Returns
    ///
    /// `true` if the condition matches and the cooldown (when applicable) has elapsed, `false` otherwise.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// // Given a compiled template `tmpl` and a country state `country`:
    /// let allowed = tmpl.can_trigger(&country, Some(0.0), 120.0);
    /// ```
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

    /// Determines whether this compiled template's condition holds for the given country.
    ///
    /// Evaluates the template's condition against `country`.
    ///
    /// # Examples
    ///
    /// ```
    /// // Given a compiled template `template` and a country state `country`:
    /// let matches = template.condition_matches(&country);
    /// if matches {
    ///     // condition is satisfied for `country`
    /// }
    /// ```
    ///
    /// # Returns
    ///
    /// `true` if the condition evaluates to true for `country`, `false` otherwise.
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

/// Computes a new resource value by applying a delta to a base and clamping it to the allowed range.
///
/// The function adds `delta` to `base`, rounds the result to the nearest integer, and constrains it to the inclusive range `[MIN_RESOURCES, MAX_RESOURCES]`.
///
/// # Examples
///
/// ```
/// let base = 50;
/// let new = clamp_resource_delta(base, 12.6);
/// assert_eq!(new, 63); // 50 + 12.6 => 62.6 rounds to 63
/// ```
fn clamp_resource_delta(base: i32, delta: f64) -> i32 {
    let candidate = (base as f64 + delta).round() as i32;
    candidate.clamp(MIN_RESOURCES, MAX_RESOURCES)
}
impl CompiledEventTemplate {
    /// Applies all compiled effects to the given country and returns any generated report messages.
    ///
    /// For each `AdjustMetric` effect, the corresponding metric on `country` is updated.
    /// For each `Report` effect, the message is formatted for `country` and collected.
    ///
    /// # Returns
    ///
    /// A vector of formatted report strings produced by `Report` effects, in the order they were applied.
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
