use anyhow::{Result, anyhow};

use super::compiler::{EventTemplateRaw, ScriptedEventState, compile_template};

const BUILTIN_TEMPLATES: &[TemplateSource] = &[
    TemplateSource::Yaml(
        "debt_crisis.yaml",
        include_str!("../../../../config/events/debt_crisis.yaml"),
    ),
    TemplateSource::Json(
        "resource_boom.json",
        include_str!("../../../../config/events/resource_boom.json"),
    ),
];

#[derive(Debug, Clone, Copy)]
enum TemplateSource {
    Yaml(&'static str, &'static str),
    Json(&'static str, &'static str),
}

pub(crate) fn load_event_templates(country_count: usize) -> Result<Vec<ScriptedEventState>> {
    load_from_sources(BUILTIN_TEMPLATES, country_count)
}

fn load_from_sources(
    sources: &[TemplateSource],
    country_count: usize,
) -> Result<Vec<ScriptedEventState>> {
    sources
        .iter()
        .enumerate()
        .map(|(idx, source)| parse_and_compile(idx, source, country_count))
        .collect()
}

fn parse_and_compile(
    index: usize,
    source: &TemplateSource,
    country_count: usize,
) -> Result<ScriptedEventState> {
    let raw = parse_template(source)?;
    compile_template(index, raw, country_count)
}

fn parse_template(source: &TemplateSource) -> Result<EventTemplateRaw> {
    match source {
        TemplateSource::Yaml(name, body) => serde_yaml::from_str::<EventTemplateRaw>(body)
            .map_err(|err| anyhow!("YAML テンプレート {} の解析に失敗しました: {}", name, err)),
        TemplateSource::Json(name, body) => serde_json::from_str::<EventTemplateRaw>(body)
            .map_err(|err| anyhow!("JSON テンプレート {} の解析に失敗しました: {}", name, err)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn load_builtin_templates_success() {
        let templates = load_event_templates(4).expect("built-in templates should load");
        assert_eq!(templates.len(), 2);
        assert_eq!(templates[0].id(), "debt_crisis");
        assert_eq!(templates[0].check_minutes(), 180);
        assert_eq!(templates[1].id(), "resource_boom");
        assert_eq!(templates[1].initial_delay_minutes(), 120);
    }

    #[test]
    fn load_from_sources_reports_parse_errors() {
        let sources = [TemplateSource::Yaml("broken.yaml", "id: [unterminated")];
        let err = load_from_sources(&sources, 1).expect_err("should propagate parse failures");
        let message = format!("{}", err);
        assert!(message.contains("解析に失敗しました"));
    }

    #[test]
    fn load_from_sources_reports_compile_errors() {
        let sources = [TemplateSource::Json(
            "invalid.json",
            r#"{
                "id": "invalid",
                "name": "Invalid",
                "description": "Invalid template",
                "condition": "approval > 10",
                "check_minutes": 0,
                "effects": []
            }"#,
        )];
        let err = load_from_sources(&sources, 1).expect_err("should propagate compile failures");
        let message = format!("{}", err);
        assert!(message.contains("check_minutes"));
    }
}
