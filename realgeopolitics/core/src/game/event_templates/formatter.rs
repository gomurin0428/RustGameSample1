#[derive(Debug, Clone)]
pub(crate) struct ScriptedEventReport {
    template: String,
    placeholders: Vec<Placeholder>,
}

#[derive(Debug, Clone)]
pub(crate) struct Placeholder {
    token: String,
    value: String,
}

impl ScriptedEventReport {
    pub(crate) fn new(template: String) -> Self {
        Self {
            template,
            placeholders: Vec::new(),
        }
    }

    pub(crate) fn add_placeholder(&mut self, token: impl Into<String>, value: impl Into<String>) {
        self.placeholders.push(Placeholder {
            token: token.into(),
            value: value.into(),
        });
    }

    pub(crate) fn template(&self) -> &str {
        &self.template
    }

    pub(crate) fn placeholders(&self) -> &[Placeholder] {
        &self.placeholders
    }
}

pub(crate) fn format_reports(reports: &[ScriptedEventReport]) -> Vec<String> {
    reports
        .iter()
        .map(|report| {
            let mut message = report.template().to_string();
            for placeholder in report.placeholders() {
                message = message.replace(&placeholder.token, &placeholder.value);
            }
            message
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_reports_replaces_all_placeholders() {
        let mut report = ScriptedEventReport::new("{country} gains {amount}".to_string());
        report.add_placeholder("{country}", "Testland");
        report.add_placeholder("{amount}", "10%");

        let formatted = format_reports(&[report]);
        assert_eq!(formatted, vec!["Testland gains 10%".to_string()]);
    }
}
