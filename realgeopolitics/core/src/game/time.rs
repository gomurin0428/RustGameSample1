use anyhow::{Result, ensure};

use super::{BASE_TICK_MINUTES, MINUTES_PER_DAY};
use crate::{CalendarDate, GameClock, ScheduledTask, Scheduler};

pub(crate) struct SimulationClock {
    clock: GameClock,
    calendar: CalendarDate,
    day_progress_minutes: u32,
    time_multiplier: f64,
    scheduler: Scheduler,
}

pub(crate) struct TickOutcome {
    pub effective_minutes: f64,
    pub scale: f64,
    pub ready_tasks: Vec<ScheduledTask>,
}

impl SimulationClock {
    pub fn new(scheduler: Scheduler) -> Self {
        Self {
            clock: GameClock::new(),
            calendar: CalendarDate::from_start(),
            day_progress_minutes: 0,
            time_multiplier: 1.0,
            scheduler,
        }
    }

    pub fn time_multiplier(&self) -> f64 {
        self.time_multiplier
    }

    pub fn set_time_multiplier(&mut self, multiplier: f64) -> Result<()> {
        ensure!(
            multiplier.is_finite() && multiplier > 0.0,
            "時間倍率は正の有限値で指定してください"
        );
        self.time_multiplier = multiplier.clamp(0.1, 5.0);
        Ok(())
    }

    pub fn calendar_date(&self) -> CalendarDate {
        self.calendar
    }

    pub fn simulation_minutes(&self) -> f64 {
        self.clock.total_minutes_f64()
    }

    pub fn next_event_in_minutes(&self) -> Option<u64> {
        let current = self.clock.total_minutes();
        self.scheduler
            .peek_next_minutes(current)
            .map(|next| next.saturating_sub(current))
    }

    pub fn advance(&mut self, minutes: f64) -> Result<TickOutcome> {
        ensure!(minutes.is_finite(), "時間が不正です");
        ensure!(minutes > 0.0, "時間は正の値で指定してください");

        let effective_minutes = minutes * self.time_multiplier;
        let advanced_minutes = self.clock.advance_minutes(effective_minutes);
        self.update_calendar(advanced_minutes);
        let scale = effective_minutes / BASE_TICK_MINUTES;
        let ready_tasks = self.scheduler.next_ready_tasks(&self.clock);

        Ok(TickOutcome {
            effective_minutes,
            scale,
            ready_tasks,
        })
    }

    fn update_calendar(&mut self, advanced_minutes: u64) {
        let mut total_days = advanced_minutes / MINUTES_PER_DAY;
        let remainder = advanced_minutes % MINUTES_PER_DAY;
        self.day_progress_minutes += remainder as u32;
        if self.day_progress_minutes as u64 >= MINUTES_PER_DAY {
            total_days += (self.day_progress_minutes as u64) / MINUTES_PER_DAY;
            self.day_progress_minutes %= MINUTES_PER_DAY as u32;
        }
        if total_days > 0 {
            self.calendar.advance_days(total_days);
        }
    }
}
