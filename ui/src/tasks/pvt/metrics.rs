//! Metric definitions and aggregation helpers for PVT summaries.

use serde::{Deserialize, Serialize};

use crate::core::timing;

use super::engine::{PvtTrial, TrialOutcome};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PvtMetrics {
    pub total_trials: usize,
    pub reacted_trials: usize,
    pub median_rt_ms: f64,
    pub mean_rt_ms: f64,
    pub sd_rt_ms: f64,
    pub p10_rt_ms: f64,
    pub p90_rt_ms: f64,
    pub lapses_ge_500ms: u32,
    pub minor_lapses_355_499ms: u32,
    pub false_starts: u32,
    pub time_on_task_slope_ms_per_min: f64,
    pub meets_min_trial_requirement: bool,
}

impl PvtMetrics {
    pub fn from_trials(trials: &[PvtTrial], false_starts: u32, min_required: usize) -> Self {
        let total_trials = trials.iter().filter(|trial| trial.is_completed()).count();

        let mut reaction_times = Vec::new();
        let mut reaction_offsets = Vec::new();
        let mut lapses_ge_500ms = 0u32;
        let mut minor_lapses = 0u32;

        for trial in trials {
            match trial.outcome {
                TrialOutcome::Reaction { rt_ms } => {
                    reaction_times.push(rt_ms);
                    let minutes = trial
                        .onset_since_start_ms
                        .map(timing::ms_to_minutes)
                        .unwrap_or_default();
                    reaction_offsets.push(minutes);

                    if rt_ms >= 500.0 {
                        lapses_ge_500ms += 1;
                    } else if (355.0..500.0).contains(&rt_ms) {
                        minor_lapses += 1;
                    }
                }
                TrialOutcome::Lapse => {
                    lapses_ge_500ms += 1;
                }
                TrialOutcome::FalseStart | TrialOutcome::Pending => {}
            }
        }

        if reaction_times.is_empty() {
            return Self {
                total_trials,
                false_starts,
                meets_min_trial_requirement: false,
                ..Default::default()
            };
        }

        let reacted_trials = reaction_times.len();

        let mut sorted_times = reaction_times.clone();
        sorted_times.sort_by(|a, b| a.partial_cmp(b).unwrap());

        let mean = mean(&reaction_times);
        let sd = std_dev(&reaction_times, mean);
        let median = percentile(&sorted_times, 0.5);
        let p10 = percentile(&sorted_times, 0.10);
        let p90 = percentile(&sorted_times, 0.90);
        let slope = slope_minutes(&reaction_offsets, &reaction_times);

        Self {
            total_trials,
            reacted_trials,
            median_rt_ms: median,
            mean_rt_ms: mean,
            sd_rt_ms: sd,
            p10_rt_ms: p10,
            p90_rt_ms: p90,
            lapses_ge_500ms,
            minor_lapses_355_499ms: minor_lapses,
            false_starts,
            time_on_task_slope_ms_per_min: slope,
            meets_min_trial_requirement: reacted_trials >= min_required,
        }
    }
}

fn mean(data: &[f64]) -> f64 {
    if data.is_empty() {
        0.0
    } else {
        data.iter().sum::<f64>() / data.len() as f64
    }
}

fn std_dev(data: &[f64], mean: f64) -> f64 {
    let n = data.len();
    if n < 2 {
        return 0.0;
    }
    let variance = data
        .iter()
        .map(|value| {
            let diff = value - mean;
            diff * diff
        })
        .sum::<f64>()
        / (n as f64 - 1.0);
    variance.sqrt()
}

fn percentile(sorted: &[f64], pct: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }

    if sorted.len() == 1 {
        return sorted[0];
    }

    let clamped_pct = pct.clamp(0.0, 1.0);
    let rank = clamped_pct * (sorted.len() as f64 - 1.0);
    let lower = rank.floor() as usize;
    let upper = rank.ceil() as usize;

    if lower == upper {
        sorted[lower]
    } else {
        let weight = rank - lower as f64;
        sorted[lower] + (sorted[upper] - sorted[lower]) * weight
    }
}

fn slope_minutes(xs_minutes: &[f64], ys_ms: &[f64]) -> f64 {
    if xs_minutes.len() < 2 || ys_ms.len() < 2 || xs_minutes.len() != ys_ms.len() {
        return 0.0;
    }

    let n = xs_minutes.len() as f64;
    let sum_x = xs_minutes.iter().sum::<f64>();
    let sum_y = ys_ms.iter().sum::<f64>();
    let sum_xy = xs_minutes
        .iter()
        .zip(ys_ms)
        .map(|(x, y)| x * y)
        .sum::<f64>();
    let sum_x2 = xs_minutes.iter().map(|x| x * x).sum::<f64>();

    let denominator = n * sum_x2 - sum_x * sum_x;
    if denominator.abs() < f64::EPSILON {
        0.0
    } else {
        (n * sum_xy - sum_x * sum_y) / denominator
    }
}
