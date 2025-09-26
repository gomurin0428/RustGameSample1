use std::cmp::Ordering;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GameClock {
    total_minutes: u64,
}

impl GameClock {
    pub fn new() -> Self {
        Self { total_minutes: 0 }
    }

    pub fn total_minutes(&self) -> u64 {
        self.total_minutes
    }

    pub fn total_minutes_f64(&self) -> f64 {
        self.total_minutes as f64
    }

    pub fn advance_minutes(&mut self, minutes: f64) -> u64 {
        let minutes_u64 = minutes.round() as i64;
        assert!(minutes_u64 >= 0, "advance_minutes に負数は指定できません");
        self.total_minutes = self
            .total_minutes
            .saturating_add(minutes_u64 as u64);
        minutes_u64 as u64
    }
}

impl Default for GameClock {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CalendarDate {
    pub year: u16,
    pub month: u8,
    pub day: u8,
}

impl CalendarDate {
    pub fn new(year: u16, month: u8, day: u8) -> Self {
        Self { year, month, day }
    }

    pub fn from_start() -> Self {
        Self::new(2025, 1, 1)
    }

    pub fn advance_days(&mut self, days: u64) {
        let mut remaining = days;
        while remaining > 0 {
            let days_in_month = days_in_month(self.year, self.month);
            if self.day as u64 + remaining <= days_in_month as u64 {
                self.day = (self.day as u64 + remaining) as u8;
                break;
            } else {
                remaining -= (days_in_month - self.day) as u64 + 1;
                self.day = 1;
                if self.month == 12 {
                    self.month = 1;
                    self.year += 1;
                } else {
                    self.month += 1;
                }
            }
        }
    }
}

#[inline]
fn days_in_month(year: u16, month: u8) -> u8 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            if is_leap_year(year) {
                29
            } else {
                28
            }
        }
        _ => 30,
    }
}

#[inline]
fn is_leap_year(year: u16) -> bool {
    (year as u32 % 4 == 0 && year as u32 % 100 != 0) || year as u32 % 400 == 0
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScheduledTime {
    pub minutes: u64,
}

impl ScheduledTime {
    pub fn new(minutes: u64) -> Self {
        Self { minutes }
    }
}

impl Ord for ScheduledTime {
    fn cmp(&self, other: &Self) -> Ordering {
        other.minutes.cmp(&self.minutes)
    }
}

impl PartialOrd for ScheduledTime {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
