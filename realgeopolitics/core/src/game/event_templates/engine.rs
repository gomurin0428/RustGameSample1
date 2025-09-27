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
    /// Constructs a ScriptedEventEngine populated with the built-in event templates.
    ///
    /// Loads the compiled built-in event templates and initializes per-template per-country
    /// instance state for `country_count`.
    ///
    /// # Parameters
    ///
    /// - `country_count`: number of countries the engine should allocate per-template state for.
    ///
    /// # Returns
    ///
    /// `Ok(engine)` containing the initialized engine on success, or an `Err` if loading the built-in
    /// templates fails.
    ///
    /// # Examples
    ///
    /// ```
    /// let mut engine = ScriptedEventEngine::from_builtin(2).unwrap();
    /// // engine is ready to execute events for 2 countries
    /// assert_eq!(engine.len(), engine.len());
    /// ```
    pub(crate) fn from_builtin(country_count: usize) -> Result<Self> {
        let templates = load_event_templates()?;
        Ok(Self::with_templates(templates, country_count))
    }

    /// Creates a ScriptedEventEngine from a list of compiled event templates and allocates per-template state for the given number of countries.
    ///
    /// The engine will contain one internal ScriptedEventInstance per template, each initialized with capacity for `country_count` countries.
    ///
    /// # Parameters
    ///
    /// - `templates`: compiled event templates to include in the engine.
    /// - `country_count`: initial number of countries to allocate per-template state for.
    ///
    /// # Returns
    ///
    /// A new `ScriptedEventEngine` populated with the provided templates and corresponding per-template per-country state.
    ///
    /// # Examples
    ///
    /// ```
    /// // Create an engine for two countries with no templates.
    /// let engine = ScriptedEventEngine::with_templates(Vec::new(), 2);
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

    /// Number of compiled event templates held by this engine.
    ///
    /// # Examples
    ///
    /// ```
    /// let engine = ScriptedEventEngine::with_templates(Vec::new(), 0);
    /// assert_eq!(engine.len(), 0);
    /// ```
    pub(crate) fn len(&self) -> usize {
        self.templates.len()
    }

    /// Returns the check interval in minutes for the template at the given index.
    ///
    /// `idx` is the index of the template within this engine.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// // Given an engine with at least one template:
    /// // let engine = ScriptedEventEngine::with_templates(templates, country_count);
    /// // let minutes = engine.check_minutes(0);
    /// ```
    pub(crate) fn check_minutes(&self, idx: usize) -> u64 {
        self.template_ref(idx).check_minutes()
    }

    /// Returns the initial delay, in minutes, for the template at the given index.
    ///
    /// # Parameters
    /// - `idx`: Template index within the engine.
    ///
    /// # Returns
    /// The template's initial delay in minutes.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let engine = ScriptedEventEngine::with_templates(templates, country_count);
    /// let delay = engine.initial_delay_minutes(0);
    /// ```
    pub(crate) fn initial_delay_minutes(&self, idx: usize) -> u64 {
        self.template_ref(idx).initial_delay_minutes()
    }

    /// Finds the index of a compiled event template whose id or name matches the provided string, case-insensitively.
    ///
    /// # Returns
    ///
    /// `Some(index)` if a template's id or name equals `id` (case-insensitive), `None` if no match.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let idx = engine.find_index("event-name");
    /// assert_eq!(idx, Some(0));
    /// ```
    pub(crate) fn find_index(&self, id: &str) -> Option<usize> {
        let needle = id.to_ascii_lowercase();
        self.templates.iter().position(|template| {
            let id_match = template.id().to_ascii_lowercase() == needle;
            let name_match = template.name().to_ascii_lowercase() == needle;
            id_match || name_match
        })
    }

    /// Find a template's description by matching the given id or name (case-insensitive).
    ///
    /// # Parameters
    ///
    /// - `id`: Template identifier or name to match (case-insensitive).
    ///
    /// # Returns
    ///
    /// `Some(description)` if a template with a matching id or name exists, `None` otherwise.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// // Given an engine, look up a template description by id or name.
    /// # use realgeopolitics::core::game::event_templates::engine::ScriptedEventEngine;
    /// # let engine = ScriptedEventEngine::with_templates(Vec::new(), 0);
    /// assert_eq!(engine.description_of("nonexistent"), None);
    /// ```
    pub(crate) fn description_of(&self, id: &str) -> Option<&str> {
        self.find_index(id)
            .map(|idx| self.template_ref(idx).description())
    }

    /// Executes the event template at the given index across all provided countries.
    ///
    /// Ensures the template index is valid, delegates execution to the per-template instance,
    /// and returns any textual reports produced by applying template effects to eligible countries.
    ///
    /// # Returns
    ///
    /// A `Vec<String>` containing textual reports generated for countries where the template fired.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// // given a mutable engine, a slice of country states, and the current time in minutes:
    /// // let mut engine = ScriptedEventEngine::with_templates(templates, country_count);
    /// // let mut countries: Vec<CountryState> = ...;
    /// // let reports = engine.execute(template_index, &mut countries, current_minutes);
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

    /// Get the compiled event template for the specified template index.
    ///
    /// # Panics
    ///
    /// Panics if `idx` is out of bounds with the message "無効なイベントテンプレートインデックス: {idx}".
    ///
    /// # Examples
    ///
    /// ```
    /// // Conceptual example: `template_ref` returns the template at the given index.
    /// // let template = engine.template_ref(0);
    /// // assert_eq!(template, &engine.templates[0]);
    /// ```
    fn template_ref(&self, idx: usize) -> &CompiledEventTemplate {
        self.templates
            .get(idx)
            .unwrap_or_else(|| panic!("無効なイベントテンプレートインデックス: {}", idx))
    }
}
impl ScriptedEventInstance {
    /// Create a new ScriptedEventInstance with per-country last-trigger timestamps initialized to `None`.
    ///
    /// # Examples
    ///
    /// ```
    /// let inst = ScriptedEventInstance::new(3);
    /// assert_eq!(inst.last_triggered.len(), 3);
    /// assert!(inst.last_triggered.iter().all(|v| v.is_none()));
    /// ```
    fn new(country_count: usize) -> Self {
        Self {
            last_triggered: vec![None; country_count],
        }
    }

    /// Ensure the per-country trigger history can track `country_count` countries.
    ///
    /// If the internal `last_triggered` vector is shorter than `country_count`, it is
    /// extended with `None` entries until it reaches that length.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// // Given a mutable ScriptedEventInstance `inst`, expand its capacity:
    /// inst.ensure_capacity(10);
    /// assert!(inst.last_triggered.len() >= 10);
    /// ```
    fn ensure_capacity(&mut self, country_count: usize) {
        if self.last_triggered.len() < country_count {
            self.last_triggered.resize(country_count, None);
        }
    }

    /// Executes a compiled event template for each country, applying effects where the template's trigger conditions are met.
    ///
    /// This updates the instance's per-country last-trigger timestamps for countries where the template was applied and
    /// returns the concatenated textual reports produced by those effects.
    ///
    /// # Examples
    ///
    /// ```
    /// // Assume `template` is a `CompiledEventTemplate` and `countries` is a `Vec<CountryState>`.
    /// let mut instance = ScriptedEventInstance::new(countries.len());
    /// let mut countries = countries;
    /// let reports = instance.execute(&template, &mut countries, 123.0);
    /// // `reports` contains any textual output produced by applied effects.
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

    /// Constructs a sample CountryState with preset political and economic attributes for use in tests.
    ///
    /// The returned country uses the provided `name` and a set of typical default values (Republic government type,
    /// initial population, GDP, various ratings, a fiscal account with a CreditRating of `A`, and default tax and budget policies).
    ///
    /// # Examples
    ///
    /// ```
    /// let c = sample_country("Utopia");
    /// let _ = c; // sample country ready for use in tests
    /// ```
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
    /// The input string must contain a valid JSON representation of an event template that matches
    /// the `EventTemplateRaw` structure.
    ///
    /// # Panics
    ///
    /// Panics if the provided JSON is invalid or does not match the expected structure.
    ///
    /// # Examples
    ///
    /// ```
    /// let _raw = parse_raw(r#"{
    ///     "id": "sample",
    ///     "name": "Sample Event",
    ///     "description": "A simple test event",
    ///     "check_minutes": 10,
    ///     "initial_delay_minutes": 0,
    ///     "conditions": [],
    ///     "effects": []
    /// }"#);
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
