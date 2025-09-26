use crate::game::state::GameState;
use crate::{ScheduledTask, TaskKind};

pub(crate) fn execute(task: &ScheduledTask, game: &mut GameState, scale: f64) -> Vec<String> {
    match task.kind {
        TaskKind::EconomicTick => game.process_economic_tick(scale),
        TaskKind::EventTrigger => game.process_event_trigger(),
        TaskKind::PolicyResolution => game.process_policy_resolution(),
        TaskKind::DiplomaticPulse => game.process_diplomatic_pulse(),
        TaskKind::ScriptedEvent(template_idx) => game.process_scripted_event(template_idx),
    }
}
