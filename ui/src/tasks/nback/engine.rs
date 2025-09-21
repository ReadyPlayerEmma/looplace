//! Engine managing the 2-back stimulus schedule and response tracking.

use rand::rngs::StdRng;
use rand::seq::SliceRandom;
use rand::SeedableRng;

use crate::core::timing::{self, InstantStamp};

use super::metrics::NBackMetrics;

const LETTER_POOL: &[char] = &[
    'B', 'C', 'D', 'F', 'G', 'H', 'J', 'K', 'M', 'P', 'Q', 'R', 'S', 'T', 'V', 'W', 'X', 'Y', 'Z',
];

/// Different run phases for the 2-back engine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunMode {
    Practice,
    Main,
}

impl RunMode {
    fn seed_tag(self) -> u64 {
        match self {
            RunMode::Practice => 0x41_5052_4143_5449_u64, // "APRACTI" (ASCII mash)
            RunMode::Main => 0x4d_4149_4e52_554e_u64,     // "MAINRUN"
        }
    }
}

/// Public configuration knobs for the task.
#[derive(Debug, Clone)]
pub struct NBackConfig {
    pub total_trials: usize,
    pub practice_trials: usize,
    pub target_ratio: f32,
    pub stimulus_ms: u64,
    pub interstimulus_interval_ms: u64,
    pub lead_in_ms: u64,
    pub response_window_ms: u64,
    pub seed: u64,
}

impl Default for NBackConfig {
    fn default() -> Self {
        Self {
            total_trials: 60,
            practice_trials: 12,
            target_ratio: 0.3,
            stimulus_ms: 500,
            interstimulus_interval_ms: 2_500,
            lead_in_ms: 750,
            response_window_ms: 3_000,
            seed: 1,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EngineState {
    Idle,
    Waiting { mode: RunMode, trial_index: usize },
    StimulusActive { mode: RunMode, trial_index: usize },
    Completed { mode: RunMode },
    Aborted,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NBackTrial {
    pub index: usize,
    pub letter: char,
    pub is_target: bool,
    pub is_lure: bool,
    pub presented_at: Option<InstantStamp>,
    pub response: Option<TrialResponse>,
    pub outcome: TrialOutcome,
}

impl NBackTrial {
    fn new(index: usize, letter: char, is_target: bool, is_lure: bool) -> Self {
        Self {
            index,
            letter,
            is_target,
            is_lure,
            presented_at: None,
            response: None,
            outcome: TrialOutcome::Pending,
        }
    }

    pub fn is_completed(&self) -> bool {
        !matches!(self.outcome, TrialOutcome::Pending)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct TrialResponse {
    pub timestamp: InstantStamp,
    pub rt_ms: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TrialOutcome {
    Pending,
    Hit { rt_ms: f64 },
    Miss,
    FalseAlarm { rt_ms: f64 },
    CorrectRejection,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResponseKind {
    Hit,
    FalseAlarm,
}

#[derive(Debug, Clone)]
pub struct ScheduledStimulus {
    pub run_id: u64,
    pub trial_index: usize,
    pub wait_ms: u64,
}

#[derive(Debug, Clone)]
pub struct ScheduledAdvance {
    pub run_id: u64,
    pub trial_index: usize,
    pub wait_ms: u64,
}

#[derive(Debug, Clone)]
pub struct TrialSchedule {
    pub stimulus: ScheduledStimulus,
    pub advance: ScheduledAdvance,
}

#[derive(Debug, Clone)]
pub enum AdvanceOutcome {
    Next(TrialSchedule),
    Completed { mode: RunMode },
    Ignored,
}

#[derive(Debug, Clone)]
pub enum ResponseOutcome {
    Recorded(ResponseKind),
    Ignored,
}

#[derive(Debug, Clone)]
pub struct NBackEngine {
    pub config: NBackConfig,
    pub state: EngineState,
    pub run_id: u64,
    trials: Vec<NBackTrial>,
    last_practice_metrics: Option<NBackMetrics>,
    last_main_metrics: Option<NBackMetrics>,
}

impl NBackEngine {
    pub fn new(config: NBackConfig) -> Self {
        Self {
            config,
            state: EngineState::Idle,
            run_id: 0,
            trials: Vec::new(),
            last_practice_metrics: None,
            last_main_metrics: None,
        }
    }

    pub fn practice_metrics(&self) -> Option<NBackMetrics> {
        self.last_practice_metrics.clone()
    }

    pub fn main_metrics(&self) -> Option<NBackMetrics> {
        self.last_main_metrics.clone()
    }

    pub fn trials(&self) -> &[NBackTrial] {
        &self.trials
    }

    pub fn start(&mut self, mode: RunMode) -> Option<TrialSchedule> {
        if matches!(
            self.state,
            EngineState::Waiting { .. } | EngineState::StimulusActive { .. }
        ) {
            return None;
        }

        self.run_id = self.run_id.wrapping_add(1);
        self.trials = self.generate_trials(mode);
        self.state = EngineState::Waiting {
            mode,
            trial_index: 0,
        };

        Some(self.schedule_current(0))
    }

    pub fn abort(&mut self) {
        self.state = EngineState::Aborted;
    }

    pub fn mark_stimulus_on(&mut self, trial_index: usize, timestamp: InstantStamp) -> bool {
        match self.state {
            EngineState::Waiting {
                mode,
                trial_index: idx,
            } if idx == trial_index => {
                if let Some(trial) = self.trials.get_mut(trial_index) {
                    trial.presented_at = Some(timestamp);
                    self.state = EngineState::StimulusActive { mode, trial_index };
                    true
                } else {
                    false
                }
            }
            _ => false,
        }
    }

    pub fn register_response(&mut self, timestamp: InstantStamp) -> ResponseOutcome {
        match self.state {
            EngineState::StimulusActive { trial_index, .. } => {
                if let Some(trial) = self.trials.get_mut(trial_index) {
                    if trial.response.is_some() || trial.presented_at.is_none() {
                        return ResponseOutcome::Ignored;
                    }

                    let onset = trial.presented_at.expect("onset recorded");
                    let rt_ms = timing::duration_ms(onset, timestamp);
                    trial.response = Some(TrialResponse { timestamp, rt_ms });

                    if trial.is_target {
                        trial.outcome = TrialOutcome::Hit { rt_ms };
                        ResponseOutcome::Recorded(ResponseKind::Hit)
                    } else {
                        trial.outcome = TrialOutcome::FalseAlarm { rt_ms };
                        ResponseOutcome::Recorded(ResponseKind::FalseAlarm)
                    }
                } else {
                    ResponseOutcome::Ignored
                }
            }
            _ => ResponseOutcome::Ignored,
        }
    }

    pub fn advance(&mut self, trial_index: usize) -> AdvanceOutcome {
        let (mode, idx) = match self.state {
            EngineState::StimulusActive {
                mode,
                trial_index: idx,
            }
            | EngineState::Waiting {
                mode,
                trial_index: idx,
            } if idx == trial_index => (mode, idx),
            _ => return AdvanceOutcome::Ignored,
        };

        if let Some(trial) = self.trials.get_mut(idx) {
            if trial.outcome == TrialOutcome::Pending {
                trial.outcome = if trial.is_target {
                    TrialOutcome::Miss
                } else {
                    TrialOutcome::CorrectRejection
                };
            }
        }

        let next_index = idx + 1;

        if next_index >= self.trials.len() {
            self.state = EngineState::Completed { mode };
            let metrics = NBackMetrics::from_trials(&self.trials);
            match mode {
                RunMode::Practice => self.last_practice_metrics = Some(metrics),
                RunMode::Main => self.last_main_metrics = Some(metrics),
            }
            AdvanceOutcome::Completed { mode }
        } else {
            self.state = EngineState::Waiting {
                mode,
                trial_index: next_index,
            };
            AdvanceOutcome::Next(self.schedule_current(next_index))
        }
    }

    fn schedule_current(&self, trial_index: usize) -> TrialSchedule {
        TrialSchedule {
            stimulus: ScheduledStimulus {
                run_id: self.run_id,
                trial_index,
                wait_ms: if trial_index == 0 {
                    self.config.lead_in_ms
                } else {
                    self.config.interstimulus_interval_ms
                },
            },
            advance: ScheduledAdvance {
                run_id: self.run_id,
                trial_index,
                wait_ms: self.config.response_window_ms,
            },
        }
    }

    fn generate_trials(&self, mode: RunMode) -> Vec<NBackTrial> {
        let length = match mode {
            RunMode::Practice => self.config.practice_trials,
            RunMode::Main => self.config.total_trials,
        };

        let mut trials = Vec::with_capacity(length);
        if length == 0 {
            return trials;
        }

        let mut rng = self.seeded_rng(mode);
        let mut letters: Vec<char> = Vec::with_capacity(length);

        for i in 0..length {
            if i < 2 {
                letters.push(random_letter(&mut rng, None));
            } else {
                letters.push(' ');
            }
        }

        let target_candidates: Vec<usize> = (2..length).collect();
        let max_targets = target_candidates.len();
        let mut target_quota =
            ((length.saturating_sub(2)) as f32 * self.config.target_ratio).round() as isize;
        if max_targets == 0 {
            target_quota = 0;
        } else {
            if target_quota <= 0 {
                target_quota = 1;
            }
            if target_quota > max_targets as isize {
                target_quota = max_targets as isize;
            }
        }
        let target_quota = target_quota.max(0) as usize;

        let mut chosen_targets = target_candidates;
        chosen_targets.shuffle(&mut rng);
        chosen_targets.truncate(target_quota);
        chosen_targets.sort_unstable();

        for idx in 2..length {
            if chosen_targets.binary_search(&idx).is_ok() {
                let letter = letters[idx - 2];
                letters[idx] = letter;
            } else {
                let previous_two = letters[idx - 2];
                letters[idx] = random_letter(&mut rng, Some(previous_two));
            }
        }

        for idx in 0..length {
            let letter = letters[idx];
            let is_target = idx >= 2 && letter == letters[idx - 2];
            let is_lure = idx >= 1 && letter == letters[idx - 1];
            trials.push(NBackTrial::new(idx, letter, is_target, is_lure));
        }

        trials
    }

    fn seeded_rng(&self, mode: RunMode) -> StdRng {
        let combined_seed = self.config.seed ^ mode.seed_tag() ^ self.run_id as u64;
        StdRng::seed_from_u64(combined_seed)
    }
}

impl Default for NBackEngine {
    fn default() -> Self {
        Self::new(NBackConfig::default())
    }
}

fn random_letter(rng: &mut StdRng, disallow: Option<char>) -> char {
    loop {
        let letter = LETTER_POOL.choose(rng).copied().unwrap_or('A');
        if disallow.map(|c| c == letter).unwrap_or(false) {
            continue;
        }
        return letter;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_sequence_respects_two_back_constraints() {
        let mut engine = NBackEngine::default();
        engine.run_id = 42;
        let trials = engine.generate_trials(RunMode::Main);

        assert_eq!(trials.len(), engine.config.total_trials);

        for idx in 2..trials.len() {
            let trial = &trials[idx];
            let two_back = &trials[idx - 2];
            if trial.is_target {
                assert_eq!(trial.letter, two_back.letter);
            } else {
                assert_ne!(trial.letter, two_back.letter);
            }
        }
    }

    #[test]
    fn run_completion_produces_metrics() {
        let mut engine = NBackEngine::new(NBackConfig {
            total_trials: 4,
            practice_trials: 4,
            target_ratio: 0.5,
            stimulus_ms: 300,
            interstimulus_interval_ms: 200,
            lead_in_ms: 10,
            response_window_ms: 500,
            seed: 9,
        });

        engine.start(RunMode::Practice).expect("schedule");
        assert_eq!(
            engine.state,
            EngineState::Waiting {
                mode: RunMode::Practice,
                trial_index: 0
            }
        );

        // Mark each trial as correct rejection by advancing without responses.
        engine.mark_stimulus_on(0, timing::now());
        assert!(matches!(engine.advance(0), AdvanceOutcome::Next(_)));
        engine.mark_stimulus_on(1, timing::now());
        assert!(matches!(engine.advance(1), AdvanceOutcome::Next(_)));
        engine.mark_stimulus_on(2, timing::now());
        assert!(matches!(engine.advance(2), AdvanceOutcome::Next(_)));
        engine.mark_stimulus_on(3, timing::now());
        assert!(matches!(
            engine.advance(3),
            AdvanceOutcome::Completed {
                mode: RunMode::Practice
            }
        ));

        let metrics = engine.practice_metrics().expect("practice metrics");
        assert_eq!(metrics.total_trials, 4);
    }
}
