//! State machine driving the Psychomotor Vigilance Task (PVT).

use rand::Rng;

use crate::core::timing::{self, InstantStamp};

use super::metrics::PvtMetrics;

const FALSE_START_THRESHOLD_MS: f64 = 100.0;

#[derive(Debug, Clone)]
pub struct PvtConfig {
    pub target_trials: usize,
    pub min_iti_ms: u64,
    pub max_iti_ms: u64,
    pub max_response_ms: u64,
    pub min_reaction_trials: usize,
}

impl Default for PvtConfig {
    fn default() -> Self {
        Self {
            target_trials: 18,
            min_iti_ms: 2_000,
            max_iti_ms: 10_000,
            max_response_ms: 1_000,
            min_reaction_trials: 12,
        }
    }
}

#[derive(Debug, Clone)]
pub struct PvtEngine {
    pub config: PvtConfig,
    pub state: EngineState,
    pub trials: Vec<PvtTrial>,
    pub run_id: u64,
    pub run_started_at: Option<InstantStamp>,
    pub run_finished_at: Option<InstantStamp>,
    total_false_starts: u32,
}

impl Default for PvtEngine {
    fn default() -> Self {
        Self::new(PvtConfig::default())
    }
}

impl PvtEngine {
    pub fn new(config: PvtConfig) -> Self {
        Self {
            config,
            state: EngineState::Idle,
            trials: Vec::new(),
            run_id: 0,
            run_started_at: None,
            run_finished_at: None,
            total_false_starts: 0,
        }
    }

    pub fn reset(&mut self) {
        self.state = EngineState::Idle;
        self.trials.clear();
        self.run_started_at = None;
        self.run_finished_at = None;
        self.total_false_starts = 0;
    }

    pub fn start(&mut self) -> Option<ScheduledStimulus> {
        if !matches!(
            self.state,
            EngineState::Idle | EngineState::Completed | EngineState::Aborted
        ) {
            return None;
        }

        self.reset();
        self.run_id = self.run_id.wrapping_add(1);
        let now = timing::now();
        self.run_started_at = Some(now);

        let first_trial = PvtTrial::new(0, self.random_iti());
        self.trials.push(first_trial);
        self.state = EngineState::Waiting { trial_index: 0 };

        Some(ScheduledStimulus {
            run_id: self.run_id,
            trial_index: 0,
            wait_ms: self.trials[0].iti_ms,
        })
    }

    pub fn abort(&mut self) {
        self.state = EngineState::Aborted;
        self.run_finished_at = Some(timing::now());
    }

    pub fn mark_stimulus_on(&mut self, trial_index: usize, timestamp: InstantStamp) -> bool {
        if !matches!(self.state, EngineState::Waiting { trial_index: idx } if idx == trial_index) {
            return false;
        }

        if let Some(run_start) = self.run_started_at {
            if let Some(trial) = self.trials.get_mut(trial_index) {
                trial.stimulus_onset = Some(timestamp);
                trial.onset_since_start_ms = Some(timing::duration_ms(run_start, timestamp));
                self.state = EngineState::StimulusActive { trial_index };
                return true;
            }
        }

        false
    }

    pub fn register_response(&mut self, timestamp: InstantStamp) -> ResponseOutcome {
        match self.state {
            EngineState::StimulusActive { trial_index } => {
                if let Some(trial) = self.trials.get_mut(trial_index) {
                    if let Some(onset) = trial.stimulus_onset {
                        let rt_ms = timing::duration_ms(onset, timestamp);
                        trial.response_at = Some(timestamp);
                        if rt_ms < FALSE_START_THRESHOLD_MS {
                            trial.outcome = TrialOutcome::FalseStart;
                            self.total_false_starts = self.total_false_starts.saturating_add(1);
                        } else {
                            trial.outcome = TrialOutcome::Reaction { rt_ms };
                        }
                        return self.schedule_next(trial_index);
                    }
                }
                ResponseOutcome::Ignored
            }
            EngineState::Waiting { trial_index } => {
                // Anticipation before stimulus.
                if let Some(trial) = self.trials.get_mut(trial_index) {
                    trial.outcome = TrialOutcome::FalseStart;
                    trial.response_at = Some(timestamp);
                }
                self.total_false_starts = self.total_false_starts.saturating_add(1);
                self.schedule_next(trial_index)
            }
            _ => ResponseOutcome::Ignored,
        }
    }

    pub fn register_timeout(&mut self, trial_index: usize) -> ResponseOutcome {
        if !matches!(self.state, EngineState::StimulusActive { trial_index: idx } if idx == trial_index)
        {
            return ResponseOutcome::Ignored;
        }

        if let Some(trial) = self.trials.get_mut(trial_index) {
            trial.outcome = TrialOutcome::Lapse;
        }
        self.schedule_next(trial_index)
    }

    pub fn metrics(&self) -> Option<PvtMetrics> {
        if !matches!(self.state, EngineState::Completed) {
            return None;
        }

        Some(PvtMetrics::from_trials(
            &self.trials,
            self.total_false_starts,
            self.config.min_reaction_trials,
        ))
    }

    fn schedule_next(&mut self, _just_finished: usize) -> ResponseOutcome {
        if self.completed_trial_count() >= self.config.target_trials {
            self.state = EngineState::Completed;
            self.run_finished_at = Some(timing::now());
            return ResponseOutcome::RunCompleted;
        }

        let next_index = self.trials.len();
        let iti = self.random_iti();
        self.trials.push(PvtTrial::new(next_index, iti));
        self.state = EngineState::Waiting {
            trial_index: next_index,
        };

        ResponseOutcome::NextScheduled(ScheduledStimulus {
            run_id: self.run_id,
            trial_index: next_index,
            wait_ms: iti,
        })
    }

    fn random_iti(&self) -> u64 {
        let mut rng = rand::thread_rng();
        rng.gen_range(self.config.min_iti_ms..=self.config.max_iti_ms)
    }

    fn completed_trial_count(&self) -> usize {
        self.trials
            .iter()
            .filter(|trial| trial.is_completed())
            .count()
    }
}

#[derive(Debug, Clone)]
pub struct PvtTrial {
    pub index: usize,
    pub iti_ms: u64,
    pub stimulus_onset: Option<InstantStamp>,
    pub onset_since_start_ms: Option<f64>,
    pub response_at: Option<InstantStamp>,
    pub outcome: TrialOutcome,
}

impl PvtTrial {
    pub fn new(index: usize, iti_ms: u64) -> Self {
        Self {
            index,
            iti_ms,
            stimulus_onset: None,
            onset_since_start_ms: None,
            response_at: None,
            outcome: TrialOutcome::Pending,
        }
    }

    pub fn is_completed(&self) -> bool {
        !matches!(self.outcome, TrialOutcome::Pending)
    }

    pub fn reaction_time_ms(&self) -> Option<f64> {
        match self.outcome {
            TrialOutcome::Reaction { rt_ms } => Some(rt_ms),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub enum TrialOutcome {
    Pending,
    Reaction { rt_ms: f64 },
    Lapse,
    FalseStart,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EngineState {
    Idle,
    Waiting { trial_index: usize },
    StimulusActive { trial_index: usize },
    Completed,
    Aborted,
}

#[derive(Debug, Clone)]
pub struct ScheduledStimulus {
    pub run_id: u64,
    pub trial_index: usize,
    pub wait_ms: u64,
}

#[derive(Debug, Clone)]
pub enum ResponseOutcome {
    NextScheduled(ScheduledStimulus),
    RunCompleted,
    Ignored,
}
