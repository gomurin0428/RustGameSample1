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
    pub(crate) fn from_builtin(country_count: usize) -> Result<Self> {
        let templates = load_event_templates()?;
        Ok(Self::with_templates(templates, country_count))
    }

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

    pub(crate) fn len(&self) -> usize {
        self.templates.len()
    }

    pub(crate) fn check_minutes(&self, idx: usize) -> u64 {
        self.template_ref(idx).check_minutes()
    }

    pub(crate) fn initial_delay_minutes(&self, idx: usize) -> u64 {
        self.template_ref(idx).initial_delay_minutes()
    }

    pub(crate) fn find_index(&self, id: &str) -> Option<usize> {
        let needle = id.to_ascii_lowercase();
        self.templates.iter().position(|template| {
            let id_match = template.id().to_ascii_lowercase() == needle;
            let name_match = template.name().to_ascii_lowercase() == needle;
            id_match || name_match
        })
    }

    pub(crate) fn description_of(&self, id: &str) -> Option<&str> {
        self.find_index(id)
            .map(|idx| self.template_ref(idx).description())
    }

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

    fn template_ref(&self, idx: usize) -> &CompiledEventTemplate {
        self.templates
            .get(idx)
            .unwrap_or_else(|| panic!("無効なイベントテンプレートインデックス: {}", idx))
    }
}
impl ScriptedEventInstance {
    fn new(country_count: usize) -> Self {
        Self {
            last_triggered: vec![None; country_count],
        }
    }

    fn ensure_capacity(&mut self, country_count: usize) {
        if self.last_triggered.len() < country_count {
            self.last_triggered.resize(country_count, None);
        }
    }

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
