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
/// This attempts to parse and compile all templates defined in `BUILTIN_TEMPLATES`; the function returns an error if any template fails to parse or compile.
///
/// # Examples
///
/// ```
/// let templates = load_event_templates().unwrap();
/// assert!(!templates.is_empty());
/// ```
///
/// # Returns
///
/// `Ok` with a vector of `CompiledEventTemplate` when all templates succeed, or an `Err` describing the first failure.
pub(crate) fn load_event_templates() -> Result<Vec<CompiledEventTemplate>> {
    load_from_sources(BUILTIN_TEMPLATES)
}

/// Loads and compiles a sequence of built-in template sources into compiled event templates.
///
/// Parses and compiles each entry in `sources` in order; if any parse or compile step fails,
/// the error is returned and no partial results are produced.
///
/// # Returns
///
/// On success, a `Vec<CompiledEventTemplate>` containing the compiled templates in the same order as `sources`.
///
/// # Examples
///
/// ```
/// let sources = [TemplateSource::Yaml("example.yaml", "---\nid: example\n")];
/// let templates = load_from_sources(&sources).unwrap();
/// assert!(!templates.is_empty());
/// ```
fn load_from_sources(sources: &[TemplateSource]) -> Result<Vec<CompiledEventTemplate>> {
    sources
        .iter()
        .enumerate()
        .map(|(idx, source)| parse_and_compile(idx, source))
        .collect()
}

/// Parses a template source and compiles it into a compiled event template.
///
/// This function parses the provided `TemplateSource` (YAML or JSON) into a raw
/// event template and then compiles it into a `CompiledEventTemplate`. Any
/// parsing or compilation error is returned.
///
/// # Returns
///
/// A `CompiledEventTemplate` on success, or an error describing the parse or
/// compilation failure.
///
/// # Examples
///
/// ```no_run
/// # use super::{TemplateSource, parse_and_compile};
/// let src = TemplateSource::Yaml("example.yaml", "id: example\ncheck_minutes: 60\n");
/// let compiled = parse_and_compile(0, &src).unwrap();
/// ```
fn parse_and_compile(index: usize, source: &TemplateSource) -> Result<CompiledEventTemplate> {
    let raw = parse_template(source)?;
    compile_template(index, raw)
}

/// Parses a template source (YAML or JSON) into an `EventTemplateRaw`.
///
/// On success returns the parsed `EventTemplateRaw`. On failure returns an `anyhow::Error` whose
/// message is localized in Japanese and includes the template name plus the underlying parse error.
///
/// # Examples
///
/// ```
/// let yaml = TemplateSource::Yaml("example.yaml", "id: test\ncheck_minutes: 10");
/// assert!(parse_template(&yaml).is_ok());
///
/// let json = TemplateSource::Json("example.json", r#"{"id":"test","check_minutes":10}"#);
/// assert!(parse_template(&json).is_ok());
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
