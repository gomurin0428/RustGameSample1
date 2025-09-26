use std::collections::{BinaryHeap, VecDeque};

use crate::time::{GameClock, ScheduledTime};

pub const ONE_YEAR_MINUTES: u64 = 365 * 24 * 60;
const IMMEDIATE_THRESHOLD_MINUTES: u64 = 10;
const COMPRESSED_BUCKET_MINUTES: u64 = 24 * 60;
const DAY_MINUTES: u64 = 24 * 60;
const WEEK_MINUTES: u64 = 7 * DAY_MINUTES;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskKind {
    EconomicTick,
    EventTrigger,
    PolicyResolution,
    DiplomaticPulse,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScheduleSpec {
    EveryMinutes(u64),
    Daily,
    Weekly,
}

impl ScheduleSpec {
    fn next_execution_minutes(&self, last_execution: u64) -> u64 {
        match self {
            ScheduleSpec::EveryMinutes(minutes) => last_execution + minutes,
            ScheduleSpec::Daily => last_execution + DAY_MINUTES,
            ScheduleSpec::Weekly => last_execution + WEEK_MINUTES,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScheduledTask {
    pub kind: TaskKind,
    pub execute_at: ScheduledTime,
    pub schedule_spec: Option<ScheduleSpec>,
}

impl ScheduledTask {
    pub fn new(kind: TaskKind, execute_at: u64) -> Self {
        Self {
            kind,
            execute_at: ScheduledTime::new(execute_at),
            schedule_spec: None,
        }
    }

    pub fn with_schedule(mut self, spec: ScheduleSpec) -> Self {
        self.schedule_spec = Some(spec);
        self
    }

    fn reschedule(&self) -> Option<Self> {
        self.schedule_spec.map(|spec| {
            let next_minutes = spec.next_execution_minutes(self.execute_at.minutes);
            let mut next_task = self.clone();
            next_task.execute_at = ScheduledTime::new(next_minutes);
            next_task
        })
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
            let promote_now = self
                .long_term_buckets
                .front()
                .map(|bucket| {
                    bucket
                        .iter()
                        .map(|task| task.execute_at.minutes)
                        .min()
                        .unwrap_or(u64::MAX)
                        <= current_minutes
                })
                .unwrap_or(false);
            if !promote_now {
                break;
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
            let task = self.short_term_tasks.pop().expect("task popped after peek");
            if let Some(next_task) = task.reschedule() {
                self.schedule(next_task);
            }
            ready.push(task);
        }

        let mut immediate_ready = Vec::new();
        while let Some(task) = self.immediate_queue.pop_front() {
            if task.execute_at.minutes <= current_minutes {
                if let Some(next_task) = task.reschedule() {
                    self.schedule(next_task);
                }
                ready.push(task);
            } else {
                immediate_ready.push(task);
            }
        }
        for task in immediate_ready {
            self.immediate_queue.push_front(task);
        }

        ready
    }
}
