#[derive(Debug, Default)]
pub(crate) struct Reporter {
    entries: Vec<String>,
}

impl Reporter {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push<S: Into<String>>(&mut self, message: S) {
        let text = message.into();
        if text.trim().is_empty() {
            return;
        }
        self.entries.push(text);
    }

    pub fn record_sector_activity(
        &mut self,
        name: &str,
        production: f64,
        demand_with_backlog: f64,
        inventory: f64,
        unmet_demand: f64,
        sales: f64,
    ) {
        if production <= f64::EPSILON && sales <= f64::EPSILON {
            return;
        }
        self.entries.push(format!(
            "{}: 生産 {:.1} / 需要 {:.1} / 在庫 {:.1} / 未充足 {:.1}",
            name, production, demand_with_backlog, inventory, unmet_demand
        ));
    }

    pub fn into_reports(self) -> Vec<String> {
        self.entries
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn push_ignores_blank_entries() {
        let mut reporter = Reporter::new();
        reporter.push("");
        reporter.push("   ");
        reporter.push("message");
        assert_eq!(reporter.into_reports(), vec!["message".to_string()]);
    }

    #[test]
    fn record_sector_activity_skips_inactive() {
        let mut reporter = Reporter::new();
        reporter.record_sector_activity("Steel", 0.0, 10.0, 0.0, 2.0, 0.0);
        assert!(reporter.into_reports().is_empty());
    }

    #[test]
    fn record_sector_activity_formats_report() {
        let mut reporter = Reporter::new();
        reporter.record_sector_activity("Steel", 120.0, 150.0, 20.0, 5.0, 100.0);
        let reports = reporter.into_reports();
        assert_eq!(reports.len(), 1);
        assert!(reports[0].contains("Steel"));
        assert!(reports[0].contains("120.0"));
    }
}
