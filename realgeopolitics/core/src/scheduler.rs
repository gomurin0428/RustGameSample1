use std::collections::{BinaryHeap, VecDeque};

use crate::time::{GameClock, ScheduledTime};

pub const ONE_YEAR_MINUTES: u64 = 365 * 24 * 60;
const IMMEDIATE_THRESHOLD_MINUTES: u64 = 10;
const COMPRESSED_BUCKET_MINUTES: u64 = 24 * 60;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskKind {
    EconomicTick,
    EventTrigger,
    PolicyResolution,
    DiplomaticPulse,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScheduledTask {
    pub kind: TaskKind,
    pub execute_at: ScheduledTime,
    pub repeat_every_minutes: Option<u64>,
}

impl ScheduledTask {
    pub fn new(kind: TaskKind, execute_at: u64) -> Self {
        Self {
            kind,
            execute_at: ScheduledTime::new(execute_at),
            repeat_every_minutes: None,
        }
    }

    pub fn with_repeat(mut self, minutes: u64) -> Self {
        self.repeat_every_minutes = Some(minutes);
        self
    }
}

impl Ord for ScheduledTask {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.execute_at.cmp(&other.execute_at)
    }
}

impl PartialOrd for ScheduledTask {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

#[derive(Default, Debug)]
pub struct Scheduler {
    immediate_queue: VecDeque<ScheduledTask>,
    short_term_tasks: BinaryHeap<ScheduledTask>,
    long_term_buckets: VecDeque<Vec<ScheduledTask>>,
}

impl Scheduler {
    pub fn new() -> Self {
        Self {
            immediate_queue: VecDeque::new(),
            short_term_tasks: BinaryHeap::new(),
            long_term_buckets: VecDeque::new(),
        }
    }

    pub fn schedule(&mut self, task: ScheduledTask) {
        if task.execute_at.minutes <= IMMEDIATE_THRESHOLD_MINUTES {
            self.immediate_queue.push_back(task);
            return;
        }

        if task.execute_at.minutes <= ONE_YEAR_MINUTES {
            self.short_term_tasks.push(task);
        } else {
            let bucket_index =
                ((task.execute_at.minutes - ONE_YEAR_MINUTES) / COMPRESSED_BUCKET_MINUTES) as usize;
            while self.long_term_buckets.len() <= bucket_index {
                self.long_term_buckets.push_back(Vec::new());
            }
            if let Some(bucket) = self.long_term_buckets.get_mut(bucket_index) {
                bucket.push(task);
            }
        }
    }

    fn promote_long_term(&mut self, current_minutes: u64) {
        if current_minutes < ONE_YEAR_MINUTES {
            return;
        }
        let elapsed_since_threshold = current_minutes - ONE_YEAR_MINUTES;
        let buckets_to_promote = (elapsed_since_threshold / COMPRESSED_BUCKET_MINUTES) as usize;
        for _ in 0..=buckets_to_promote {
            if let Some(bucket) = self.long_term_buckets.front() {
                let earliest_time = bucket
                    .iter()
                    .map(|task| task.execute_at.minutes)
                    .min()
                    .unwrap_or(u64::MAX);
                if earliest_time > current_minutes {
                    break;
                }
            }
            if let Some(bucket) = self.long_term_buckets.pop_front() {
                for task in bucket {
                    if task.execute_at.minutes > current_minutes {
                        self.short_term_tasks.push(task);
                    } else {
                        self.immediate_queue.push_back(task);
                    }
                }
            } else {
                break;
            }
        }
    }

    pub fn next_ready_tasks(&mut self, clock: &GameClock) -> Vec<ScheduledTask> {
        let current_minutes = clock.total_minutes();
        self.promote_long_term(current_minutes);

        let mut ready = Vec::new();
        while let Some(task) = self.short_term_tasks.peek() {
            if task.execute_at.minutes > current_minutes {
                break;
            }
            if let Some(mut task) = self.short_term_tasks.pop() {
                ready.push(task.clone());
                if let Some(interval) = task.repeat_every_minutes {
                    task.execute_at = ScheduledTime::new(task.execute_at.minutes + interval);
                    self.schedule(task);
                }
            }
        }

        while let Some(task) = self.immediate_queue.pop_front() {
            ready.push(task);
        }

        ready
    }
}
