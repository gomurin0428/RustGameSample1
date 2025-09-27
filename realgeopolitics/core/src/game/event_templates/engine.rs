use anyhow::Result;

use crate::game::country::CountryState;

use super::compiler::CompiledEventTemplate;
use super::loader::load_event_templates;

#[derive(Debug)]
pub(crate) struct ScriptedEventEngine {
    templates: Vec<CompiledEventTemplate>,
    instances: Vec<ScriptedEventInstance>,
}

#[derive(Debug)]
struct ScriptedEventInstance {
    last_triggered: Vec<Option<f64>>,
}
impl ScriptedEventEngine {
    /// Creates a ScriptedEventEngine populated with the built-in compiled event templates for the specified number of countries.
    ///
    /// Returns an engine whose templates are loaded from the built-in source and whose per-template state is initialized for `country_count`.
    ///
    /// # Errors
    ///
    /// Returns an error if loading the built-in event templates fails.
    ///
    /// # Examples
    ///
    /// ```
    /// let engine = ScriptedEventEngine::from_builtin(3).unwrap();
    /// assert!(engine.len() >= 0);
    /// ```
    pub(crate) fn from_builtin(country_count: usize) -> Result<Self> {
        let templates = load_event_templates()?;
        Ok(Self::with_templates(templates, country_count))
    }

    /// Constructs a ScriptedEventEngine from compiled templates and initializes per-template,
    /// per-country execution state for the given number of countries.
    ///
    /// # Examples
    ///
    /// ```
    /// let engine = ScriptedEventEngine::with_templates(vec![], 3);
    /// assert_eq!(engine.len(), 0);
    /// ```
    pub(super) fn with_templates(
        templates: Vec<CompiledEventTemplate>,
        country_count: usize,
    ) -> Self {
        let instances = templates
            .iter()
            .map(|_| ScriptedEventInstance::new(country_count))
            .collect();
        Self {
            templates,
            instances,
        }
    }

    /// Reports the number of compiled event templates managed by the engine.
    ///
    /// # Examples
    ///
    /// ```
    /// let engine = ScriptedEventEngine::with_templates(Vec::new(), 0);
    /// assert_eq!(engine.len(), 0);
    /// ```
    ///
    /// The returned value is the count of templates held in the engine.
    pub(crate) fn len(&self) -> usize {
        self.templates.len()
    }

    /// Gets the check interval in minutes for the template at the given index.
    ///
    /// The returned value is the number of minutes between automatic checks for that template.
    ///
    /// # Panics
    ///
    /// Panics if `idx` is not a valid template index.
    ///
    /// # Examples
    ///
    /// ```
    /// let minutes = engine.check_minutes(0);
    /// assert!(minutes > 0);
    /// ```
    pub(crate) fn check_minutes(&self, idx: usize) -> u64 {
        self.template_ref(idx).check_minutes()
    }

    /// Returns the initial delay, in minutes, before the template at the given index is first eligible to run.
    ///
    /// # Returns
    ///
    /// The number of minutes of initial delay for the template at `idx`.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// let engine = ScriptedEventEngine::with_templates(templates, country_count);
    /// let delay = engine.initial_delay_minutes(0);
    /// println!("Initial delay: {} minutes", delay);
    /// ```
    pub(crate) fn initial_delay_minutes(&self, idx: usize) -> u64 {
        self.template_ref(idx).initial_delay_minutes()
    }

    /// Finds the index of a template whose id or name matches the provided string, case-insensitively.
    ///
    /// The search compares the given `id` against each template's `id()` and `name()` using
    /// ASCII case-insensitive comparison and returns the position of the first match.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let idx = engine.find_index("approval_push");
    /// if let Some(i) = idx {
    ///     // use index `i` to access the template
    /// }
    /// ```
    pub(crate) fn find_index(&self, id: &str) -> Option<usize> {
        let needle = id.to_ascii_lowercase();
        self.templates.iter().position(|template| {
            let id_match = template.id().to_ascii_lowercase() == needle;
            let name_match = template.name().to_ascii_lowercase() == needle;
            id_match || name_match
        })
    }

    /// Returns the description text for the template matching the given id or name (case-insensitive).
    ///
    /// # Parameters
    ///
    /// - `id`: Template id or name to search (case-insensitive).
    ///
    /// # Returns
    ///
    /// The template's description if a matching template is found, or `None`.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// // assuming `engine` is a ScriptedEventEngine populated with templates
    /// assert_eq!(
    ///     engine.description_of("approval_push"),
    ///     Some("Approval push event that increases approval")
    /// );
    /// ```
    pub(crate) fn description_of(&self, id: &str) -> Option<&str> {
        self.find_index(id)
            .map(|idx| self.template_ref(idx).description())
    }

    /// Executes the template at the given index against all provided countries and returns any generated reports.
    ///
    /// - `idx`: Index of the compiled event template to run.
    /// - `countries`: Mutable slice of country states to apply the template to.
    /// - `current_minutes`: Current simulation time in minutes used for cooldown checks.
    ///
    /// # Returns
    ///
    /// A vector of report strings produced by applying the template to countries; empty if no triggers occurred.
    ///
    /// # Panics
    ///
    /// Panics if `idx` is out of bounds for the engine's templates or instances.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// // Assuming `engine` is a ScriptedEventEngine with at least one template and `countries` is a mutable slice:
    /// // let mut engine = ScriptedEventEngine::with_templates(templates, country_count);
    /// // let mut countries = vec![sample_country("A")];
    /// // let reports = engine.execute(0, &mut countries, 1234.0);
    /// ```
    pub(crate) fn execute(
        &mut self,
        idx: usize,
        countries: &mut [CountryState],
        current_minutes: f64,
    ) -> Vec<String> {
        let (templates, instances) = (&self.templates, &mut self.instances);
        let template = templates
            .get(idx)
            .unwrap_or_else(|| panic!("無効なイベントテンプレートインデックス: {}", idx));
        let instance = instances
            .get_mut(idx)
            .unwrap_or_else(|| panic!("無効なイベントテンプレートインデックス: {}", idx));
        instance.execute(template, countries, current_minutes)
    }

    /// Returns a reference to the compiled event template at the specified index.
    ///
    /// # Panics
    ///
    /// Panics if `idx` is out of bounds. The panic message is:
    /// "無効なイベントテンプレートインデックス: {idx}".
    ///
    /// # Examples
    ///
    /// ```no_run
    /// let engine = ScriptedEventEngine::with_templates(vec![/* CompiledEventTemplate */], 1);
    /// let template = engine.template_ref(0);
    /// ```
    fn template_ref(&self, idx: usize) -> &CompiledEventTemplate {
        self.templates
            .get(idx)
            .unwrap_or_else(|| panic!("無効なイベントテンプレートインデックス: {}", idx))
    }
}
impl ScriptedEventInstance {
    /// Creates a new ScriptedEventInstance with per-country last-trigger times initialized to `None`.
    ///
    /// The returned instance has `country_count` entries of `None`, indicating that no country has
    /// triggered an event yet.
    ///
    /// # Examples
    ///
    /// ```
    /// let instance = ScriptedEventInstance::new(3);
    /// // instance now tracks three countries, each with no prior trigger time
    /// ```
    fn new(country_count: usize) -> Self {
        Self {
            last_triggered: vec![None; country_count],
        }
    }

    /// Ensure the internal per-country cooldown vector can hold at least `country_count` entries.
    ///
    /// Expands `last_triggered` with `None` entries if it is shorter than `country_count`.
    ///
    /// # Examples
    ///
    /// ```
    /// let mut inst = ScriptedEventInstance::new(1);
    /// inst.ensure_capacity(3);
    /// assert_eq!(inst.last_triggered.len(), 3);
    /// ```
    fn ensure_capacity(&mut self, country_count: usize) {
        if self.last_triggered.len() < country_count {
            self.last_triggered.resize(country_count, None);
        }
    }

    /// Executes a compiled event template against all provided countries, applying effects where the template may trigger and recording per-country last-triggered times.
    ///
    /// The method ensures internal per-country state is large enough for `countries`, checks each country with `template.can_trigger` using its last-triggered timestamp and `current_minutes`, applies effects for countries that may trigger, updates their last-triggered time to `current_minutes`, and returns all reports produced by applied effects.
    ///
    /// # Parameters
    ///
    /// - `template`: The compiled event template to evaluate and apply.
    /// - `countries`: Mutable slice of country states that may be modified by the template's effects.
    /// - `current_minutes`: Current time in minutes used for cooldown and trigger checks.
    ///
    /// # Returns
    ///
    /// A `Vec<String>` containing all reports produced by applying the template's effects to countries.
    ///
    /// # Examples
    ///
    /// ```
    /// # // Helpers such as `sample_country` and `compile_sample_template` are available in this crate's tests.
    /// use crate::event_templates::{ScriptedEventInstance, CompiledEventTemplate};
    ///
    /// let mut instance = ScriptedEventInstance::new(1);
    /// let mut countries = vec![sample_country("Country A")];
    /// let template: CompiledEventTemplate = compile_sample_template(); // a template that may produce reports
    ///
    /// let reports = instance.execute(&template, &mut countries, 0.0);
    /// // `reports` contains messages produced by the template's applied effects
    /// ```
    fn execute(
        &mut self,
        template: &CompiledEventTemplate,
        countries: &mut [CountryState],
        current_minutes: f64,
    ) -> Vec<String> {
        self.ensure_capacity(countries.len());
        let mut reports = Vec::new();
        for (idx, country) in countries.iter_mut().enumerate() {
            if !template.can_trigger(country, self.last_triggered[idx], current_minutes) {
                continue;
            }
            let mut local_reports = template.apply_effects(country);
            reports.append(&mut local_reports);
            self.last_triggered[idx] = Some(current_minutes);
        }
        reports
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::country::{BudgetAllocation, CountryState};
    use crate::game::economy::CreditRating;
    use crate::game::economy::{FiscalAccount, TaxPolicy};
    use super::super::compiler::{EventTemplateRaw, compile_template};
    use serde_json;

    fn sample_country(name: &str) -> CountryState {
        CountryState::new(
            name.to_string(),
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

    /// Parses a JSON string into an `EventTemplateRaw`.
    ///
    /// # Panics
    ///
    /// Panics if the provided JSON is not valid for `EventTemplateRaw`.
    ///
    /// # Returns
    ///
    /// `EventTemplateRaw` represented by the provided JSON.
    ///
    /// # Examples
    ///
    /// ```
    /// let raw_json = r#"{ "id": "example", "name": "Example Event" }"#;
    /// let template = parse_raw(raw_json);
    /// // `template` is an `EventTemplateRaw` constructed from `raw_json`.
    /// ```
    fn parse_raw(json: &str) -> EventTemplateRaw {
        serde_json::from_str(json).expect("template json should be valid")
    }

    #[test]
    fn engine_execute_applies_effects_and_respects_cooldown() {
        let raw = parse_raw(
            r#"{
                "id": "approval_push",
                "name": "Approval Push",
                "description": "desc",
                "condition": "approval >= 40",
                "check_minutes": 60,
                "initial_delay_minutes": 5,
                "cooldown_minutes": 120,
                "effects": [
                    { "type": "adjust_metric", "metric": "approval", "delta": 10.0 },
                    { "type": "report", "message": "{country} improved approval" }
                ]
            }"#,
        );
        let template = compile_template(0, raw).expect("valid template should compile");
        let mut engine = ScriptedEventEngine::with_templates(vec![template], 1);

        assert_eq!(engine.len(), 1);
        assert_eq!(engine.check_minutes(0), 60);
        assert_eq!(engine.initial_delay_minutes(0), 5);
        assert_eq!(engine.description_of("approval_push"), Some("desc"));

        let mut countries = vec![sample_country("Testland")];
        let reports = engine.execute(0, &mut countries, 300.0);
        assert_eq!(reports.len(), 1);
        assert_eq!(reports[0], "Testland improved approval");
        assert_eq!(countries[0].approval, 55);

        let reports_second = engine.execute(0, &mut countries, 360.0);
        assert!(reports_second.is_empty());
        assert_eq!(countries[0].approval, 55);
    }
    #[test]
    fn engine_expands_instance_capacity_for_additional_countries() {
        let raw = parse_raw(
            r#"{
                "id": "broad_effect",
                "name": "Broad Effect",
                "description": "desc",
                "condition": "approval >= 0",
                "check_minutes": 30,
                "initial_delay_minutes": 0,
                "cooldown_minutes": 30,
                "effects": [
                    { "type": "adjust_metric", "metric": "approval", "delta": 5.0 }
                ]
            }"#,
        );
        let template = compile_template(0, raw).expect("template should compile");
        let mut engine = ScriptedEventEngine::with_templates(vec![template], 1);

        let mut countries = vec![sample_country("Alpha"), sample_country("Beta")];
        let baseline_alpha = countries[0].approval;
        let baseline_beta = countries[1].approval;
        let reports = engine.execute(0, &mut countries, 45.0);
        assert!(reports.is_empty());
        assert_eq!(countries[0].approval, baseline_alpha + 5);
        assert_eq!(countries[1].approval, baseline_beta + 5);
    }
}
