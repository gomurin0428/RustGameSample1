use std::collections::{BinaryHeap, VecDeque};

use crate::time::{GameClock, ScheduledTime};

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
    future_tasks: BinaryHeap<ScheduledTask>,
}

impl Scheduler {
    pub fn new() -> Self {
        Self {
            immediate_queue: VecDeque::new(),
            future_tasks: BinaryHeap::new(),
        }
    }

    pub fn schedule(&mut self, task: ScheduledTask) {
        if task.execute_at.minutes <= 10 {
            self.immediate_queue.push_back(task);
        } else {
            self.future_tasks.push(task);
        }
    }

    pub fn next_ready_tasks(&mut self, clock: &GameClock) -> Vec<ScheduledTask> {
        let current_minutes = clock.total_minutes();
        let mut ready = Vec::new();
        while let Some(task) = self.future_tasks.peek() {
            if task.execute_at.minutes > current_minutes {
                break;
            }
            if let Some(mut task) = self.future_tasks.pop() {
                ready.push(task.clone());
                if let Some(interval) = task.repeat_every_minutes {
                    task.execute_at = ScheduledTime::new(task.execute_at.minutes + interval);
                    self.future_tasks.push(task);
                }
            }
        }

        while let Some(task) = self.immediate_queue.pop_front() {
            ready.push(task);
        }

        ready
    }
}
