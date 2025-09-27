use std::collections::{HashMap, hash_map::Entry};

use super::model::{SectorId, SectorMetrics};

#[derive(Debug, Default, Clone, Copy)]
pub(crate) struct MetricsTotals {
    revenue: f64,
    cost: f64,
    gdp: f64,
}

impl MetricsTotals {
    pub fn revenue(self) -> f64 {
        self.revenue
    }

    pub fn cost(self) -> f64 {
        self.cost
    }

    pub fn gdp(self) -> f64 {
        self.gdp
    }

    fn accumulate(&mut self, metrics: &SectorMetrics) {
        self.revenue += metrics.revenue;
        self.cost += metrics.cost;
        self.gdp += metrics.revenue - metrics.cost;
    }

    fn retract(&mut self, metrics: &SectorMetrics) {
        self.revenue -= metrics.revenue;
        self.cost -= metrics.cost;
        self.gdp -= metrics.revenue - metrics.cost;
    }
}

#[derive(Debug, Default, Clone)]
pub(crate) struct SectorMetricsStore {
    entries: HashMap<SectorId, SectorMetrics>,
    totals: MetricsTotals,
}

impl SectorMetricsStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn begin_tick(&mut self) {
        self.entries.clear();
        self.totals = MetricsTotals::default();
    }

    pub fn record(&mut self, id: SectorId, metrics: SectorMetrics) {
        match self.entries.entry(id) {
            Entry::Occupied(mut slot) => {
                self.totals.retract(slot.get());
                *slot.get_mut() = metrics;
                self.totals.accumulate(slot.get());
            }
            Entry::Vacant(slot) => {
                let value = slot.insert(metrics);
                self.totals.accumulate(value);
            }
        }
    }

    pub fn totals(&self) -> MetricsTotals {
        self.totals
    }

    pub fn metrics(&self) -> &HashMap<SectorId, SectorMetrics> {
        &self.entries
    }

    pub fn get(&self, id: &SectorId) -> Option<&SectorMetrics> {
        self.entries.get(id)
    }

    pub fn snapshot(&self) -> HashMap<SectorId, SectorMetrics> {
        self.entries.clone()
    }

    #[cfg(test)]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::economy::IndustryCategory;

    fn sample_id(key: &str) -> SectorId {
        SectorId::new(IndustryCategory::Primary, key)
    }

    fn sample_metrics(output: f64, revenue: f64, cost: f64) -> SectorMetrics {
        SectorMetrics {
            output,
            revenue,
            cost,
            sales: output,
            demand: output,
            inventory: 0.0,
            unmet_demand: 0.0,
        }
    }

    #[test]
    fn begin_tick_clears_previous_entries() {
        let mut store = SectorMetricsStore::new();
        store.record(sample_id("iron"), sample_metrics(10.0, 50.0, 30.0));
        assert!(!store.is_empty());
        store.begin_tick();
        assert!(store.is_empty());
        assert_eq!(store.totals().revenue(), 0.0);
    }

    #[test]
    fn record_updates_totals() {
        let mut store = SectorMetricsStore::new();
        store.begin_tick();
        store.record(sample_id("grain"), sample_metrics(10.0, 40.0, 22.0));
        store.record(sample_id("steel"), sample_metrics(5.0, 30.0, 15.0));

        let totals = store.totals();
        assert!((totals.revenue() - 70.0).abs() < 1e-6);
        assert!((totals.cost() - 37.0).abs() < 1e-6);
        assert!((totals.gdp() - 33.0).abs() < 1e-6);
    }

    #[test]
    fn record_replaces_existing_entry_without_double_counting() {
        let mut store = SectorMetricsStore::new();
        store.begin_tick();
        let id = sample_id("grain");
        store.record(id.clone(), sample_metrics(10.0, 40.0, 30.0));
        store.record(id.clone(), sample_metrics(20.0, 60.0, 35.0));

        let totals = store.totals();
        assert!((totals.revenue() - 60.0).abs() < 1e-6);
        assert!((totals.cost() - 35.0).abs() < 1e-6);
        assert!((totals.gdp() - 25.0).abs() < 1e-6);
        let snapshot = store.snapshot();
        assert_eq!(snapshot.len(), 1);
        assert!((snapshot[&id].output - 20.0).abs() < 1e-6);
    }
}
