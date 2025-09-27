use anyhow::{Result, anyhow};

use super::compiler::{CompiledEventTemplate, EventTemplateRaw, compile_template};

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

/// Load and compile the built-in event templates.
///
/// This parses the embedded YAML/JSON template sources and compiles each into a
/// `CompiledEventTemplate`.
///
/// # Returns
///
/// `Ok` with a vector of compiled event templates on success. `Err` if any
/// template fails to parse or compile; the error message includes the template
/// name and the failure reason.
///
/// # Examples
///
/// ```rust
/// let templates = load_event_templates().unwrap();
/// assert!(!templates.is_empty());
/// ```
pub(crate) fn load_event_templates() -> Result<Vec<CompiledEventTemplate>> {
    load_from_sources(BUILTIN_TEMPLATES)
}

/// Compile event templates from the provided sources, preserving source order.
///
/// Returns a vector with one `CompiledEventTemplate` per source when all sources
/// parse and compile successfully; returns the first encountered error otherwise.
///
/// # Examples
///
/// ```
/// let templates = load_from_sources(&[]).unwrap();
/// assert!(templates.is_empty());
/// ```
fn load_from_sources(sources: &[TemplateSource]) -> Result<Vec<CompiledEventTemplate>> {
    sources
        .iter()
        .enumerate()
        .map(|(idx, source)| parse_and_compile(idx, source))
        .collect()
}

/// Parses a template source and compiles it into a `CompiledEventTemplate`.
///
/// # Examples
///
/// ```
/// use realgeopolitics::game::event_templates::loader::{parse_and_compile, TemplateSource};
/// // Minimal YAML matching the expected EventTemplateRaw fields.
/// let src = TemplateSource::Yaml(
///     "example",
///     "id: example\ntitle: Example Event\ncheck_minutes: 60\ninitial_delay_minutes: 10\n",
/// );
/// let compiled = parse_and_compile(0, &src).unwrap();
/// assert_eq!(compiled.id, "example");
/// ```
///
/// # Returns
///
/// `CompiledEventTemplate` when parsing and compilation succeed.
fn parse_and_compile(index: usize, source: &TemplateSource) -> Result<CompiledEventTemplate> {
    let raw = parse_template(source)?;
    compile_template(index, raw)
}

/// Parses a template source into an EventTemplateRaw.
///
/// Returns an error if the source cannot be parsed; the error message includes the template name and format (YAML or JSON).
///
/// # Examples
///
/// ```
/// let src = TemplateSource::Yaml("example", "id: example\ncheck_minutes: 10");
/// assert!(parse_template(&src).is_ok());
/// ```
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
        let templates = load_event_templates().expect("built-in templates should load");
        assert_eq!(templates.len(), 2);
        assert_eq!(templates[0].id(), "debt_crisis");
        assert_eq!(templates[0].check_minutes(), 180);
        assert_eq!(templates[1].id(), "resource_boom");
        assert_eq!(templates[1].initial_delay_minutes(), 120);
    }

    #[test]
    fn load_from_sources_reports_parse_errors() {
        let sources = [TemplateSource::Yaml("broken.yaml", "id: [unterminated")];
        let err = load_from_sources(&sources).expect_err("should propagate parse failures");
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
        let err = load_from_sources(&sources).expect_err("should propagate compile failures");
        let message = format!("{}", err);
        assert!(message.contains("check_minutes"));
    }
}
