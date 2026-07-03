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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tasks::pvt::engine::{PvtTrial, TrialOutcome};

    /// A completed trial with the given outcome, presented `onset_ms` after
    /// session start.
    fn trial(index: usize, outcome: TrialOutcome, onset_ms: f64) -> PvtTrial {
        let mut t = PvtTrial::new(index, 2000);
        t.onset_since_start_ms = Some(onset_ms);
        t.outcome = outcome;
        t
    }

    fn reaction(index: usize, rt_ms: f64, onset_ms: f64) -> PvtTrial {
        trial(index, TrialOutcome::Reaction { rt_ms }, onset_ms)
    }

    fn assert_close(actual: f64, expected: f64, label: &str) {
        assert!(
            (actual - expected).abs() < 1e-9,
            "{label}: expected {expected}, got {actual}"
        );
    }

    #[test]
    fn stats_match_hand_computed_values() {
        // RTs 200/250/300/350/400 at one-minute spacing: every stat is
        // hand-checkable.
        let trials: Vec<PvtTrial> = [200.0, 250.0, 300.0, 350.0, 400.0]
            .iter()
            .enumerate()
            .map(|(i, rt)| reaction(i, *rt, i as f64 * 60_000.0))
            .collect();

        let m = PvtMetrics::from_trials(&trials, 0, 5);
        assert_eq!(m.total_trials, 5);
        assert_eq!(m.reacted_trials, 5);
        assert_close(m.mean_rt_ms, 300.0, "mean");
        assert_close(m.median_rt_ms, 300.0, "median");
        // Sample SD: sqrt((100² + 50² + 0 + 50² + 100²) / 4) = sqrt(6250).
        assert_close(m.sd_rt_ms, 6250.0_f64.sqrt(), "sd");
        // Interpolated percentiles: rank = pct * (n-1).
        assert_close(m.p10_rt_ms, 220.0, "p10"); // rank 0.4 → 200 + 0.4·50
        assert_close(m.p90_rt_ms, 380.0, "p90"); // rank 3.6 → 350 + 0.6·50
        // RT climbs 50 ms per minute of time-on-task.
        assert_close(m.time_on_task_slope_ms_per_min, 50.0, "slope");
        assert!(m.meets_min_trial_requirement);
        assert_eq!(m.lapses_ge_500ms, 0);
        assert_eq!(m.minor_lapses_355_499ms, 1); // the 400 ms reaction
    }

    #[test]
    fn lapse_thresholds_are_exact_at_boundaries() {
        // 354.9 → clean; 355.0 and 499.9 → minor; 500.0 → major. A slow
        // reaction is a lapse but still counts as a reacted trial.
        let trials = vec![
            reaction(0, 354.9, 0.0),
            reaction(1, 355.0, 1000.0),
            reaction(2, 499.9, 2000.0),
            reaction(3, 500.0, 3000.0),
        ];

        let m = PvtMetrics::from_trials(&trials, 0, 1);
        assert_eq!(m.reacted_trials, 4);
        assert_eq!(m.minor_lapses_355_499ms, 2);
        assert_eq!(m.lapses_ge_500ms, 1);
    }

    #[test]
    fn timeout_lapses_count_without_polluting_rt_stats() {
        // A full timeout (TrialOutcome::Lapse) is a major lapse but has no RT,
        // so the stats must come from the reactions alone.
        let trials = vec![
            reaction(0, 300.0, 0.0),
            trial(1, TrialOutcome::Lapse, 60_000.0),
            reaction(2, 300.0, 120_000.0),
        ];

        let m = PvtMetrics::from_trials(&trials, 0, 1);
        assert_eq!(m.total_trials, 3);
        assert_eq!(m.reacted_trials, 2);
        assert_eq!(m.lapses_ge_500ms, 1);
        assert_close(m.mean_rt_ms, 300.0, "mean");
        assert_close(m.sd_rt_ms, 0.0, "sd");
        assert_close(m.time_on_task_slope_ms_per_min, 0.0, "slope");
    }

    #[test]
    fn false_starts_pass_through_and_contribute_no_rt() {
        let trials = vec![
            trial(0, TrialOutcome::FalseStart, 0.0),
            reaction(1, 280.0, 5000.0),
        ];

        let m = PvtMetrics::from_trials(&trials, 3, 1);
        assert_eq!(m.false_starts, 3);
        assert_eq!(m.reacted_trials, 1);
        assert_close(m.median_rt_ms, 280.0, "median");
        // Single reaction: dispersion and slope are degenerate → 0.
        assert_close(m.sd_rt_ms, 0.0, "sd");
        assert_close(m.time_on_task_slope_ms_per_min, 0.0, "slope");
    }

    #[test]
    fn no_reactions_yields_zeroed_metrics() {
        // Pending trials are not completed; false starts still surface.
        let trials = vec![
            trial(0, TrialOutcome::Pending, 0.0),
            trial(1, TrialOutcome::FalseStart, 1000.0),
        ];

        let m = PvtMetrics::from_trials(&trials, 1, 1);
        assert_eq!(m.total_trials, 1); // the false start completed; pending didn't
        assert_eq!(m.reacted_trials, 0);
        assert_eq!(m.false_starts, 1);
        assert!(!m.meets_min_trial_requirement);
        assert_close(m.median_rt_ms, 0.0, "median");

        let empty = PvtMetrics::from_trials(&[], 0, 1);
        assert_eq!(empty.total_trials, 0);
        assert!(!empty.meets_min_trial_requirement);
    }

    #[test]
    fn min_trial_requirement_counts_reactions_only() {
        // 2 reactions + 1 timeout lapse: min_required=3 must NOT be met (lapses
        // don't count toward the reacted minimum), min_required=2 must be.
        let trials = vec![
            reaction(0, 300.0, 0.0),
            trial(1, TrialOutcome::Lapse, 1000.0),
            reaction(2, 320.0, 2000.0),
        ];

        assert!(!PvtMetrics::from_trials(&trials, 0, 3).meets_min_trial_requirement);
        assert!(PvtMetrics::from_trials(&trials, 0, 2).meets_min_trial_requirement);
    }

    #[test]
    fn median_interpolates_even_counts() {
        let trials = vec![
            reaction(0, 200.0, 0.0),
            reaction(1, 300.0, 1000.0),
            reaction(2, 400.0, 2000.0),
            reaction(3, 500.0, 3000.0),
        ];
        let m = PvtMetrics::from_trials(&trials, 0, 1);
        assert_close(m.median_rt_ms, 350.0, "median");
    }

    #[test]
    fn slope_is_zero_when_all_onsets_coincide() {
        // Degenerate regression (zero x-variance) must not divide by ~0.
        let trials = vec![reaction(0, 300.0, 0.0), reaction(1, 400.0, 0.0)];
        let m = PvtMetrics::from_trials(&trials, 0, 1);
        assert_close(m.time_on_task_slope_ms_per_min, 0.0, "slope");
    }
}
