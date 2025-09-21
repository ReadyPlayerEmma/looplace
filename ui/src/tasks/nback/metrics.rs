//! Metric definitions and aggregation helpers for 2-back runs.

use serde::{Deserialize, Serialize};

use super::engine::{NBackTrial, TrialOutcome};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct NBackMetrics {
    pub total_trials: usize,
    pub target_trials: usize,
    pub non_target_trials: usize,
    pub hits: u32,
    pub misses: u32,
    pub false_alarms: u32,
    pub correct_rejections: u32,
    pub hit_rate: f64,
    pub false_alarm_rate: f64,
    pub accuracy: f64,
    pub d_prime: f64,
    pub criterion: f64,
    pub mean_hit_rt_ms: f64,
    pub median_hit_rt_ms: f64,
    pub sd_hit_rt_ms: f64,
    pub p10_hit_rt_ms: f64,
    pub p90_hit_rt_ms: f64,
    pub response_count: u32,
}

impl NBackMetrics {
    pub fn empty() -> Self {
        Self::default()
    }

    pub fn from_trials(trials: &[NBackTrial]) -> Self {
        let total_trials = trials.len();
        if total_trials == 0 {
            return Self::default();
        }

        let mut target_trials = 0usize;
        let mut non_target_trials = 0usize;
        let mut hits = 0u32;
        let mut misses = 0u32;
        let mut false_alarms = 0u32;
        let mut correct_rejections = 0u32;
        let mut hit_rts = Vec::new();

        for trial in trials {
            if trial.is_target {
                target_trials += 1;
            } else {
                non_target_trials += 1;
            }

            match trial.outcome {
                TrialOutcome::Hit { rt_ms } => {
                    hits = hits.saturating_add(1);
                    hit_rts.push(rt_ms);
                }
                TrialOutcome::Miss => {
                    misses = misses.saturating_add(1);
                }
                TrialOutcome::FalseAlarm { .. } => {
                    false_alarms = false_alarms.saturating_add(1);
                }
                TrialOutcome::CorrectRejection => {
                    correct_rejections = correct_rejections.saturating_add(1);
                }
                TrialOutcome::Pending => {}
            }
        }

        let response_count = hits + false_alarms;

        let mut mean_hit_rt_ms = 0.0;
        let mut median_hit_rt_ms = 0.0;
        let mut sd_hit_rt_ms = 0.0;
        let mut p10_hit_rt_ms = 0.0;
        let mut p90_hit_rt_ms = 0.0;

        if !hit_rts.is_empty() {
            hit_rts.sort_by(|a, b| a.partial_cmp(b).unwrap());
            mean_hit_rt_ms = mean(&hit_rts);
            median_hit_rt_ms = percentile(&hit_rts, 0.5);
            sd_hit_rt_ms = std_dev(&hit_rts, mean_hit_rt_ms);
            p10_hit_rt_ms = percentile(&hit_rts, 0.10);
            p90_hit_rt_ms = percentile(&hit_rts, 0.90);
        }

        let raw_hit_rate = if target_trials > 0 {
            hits as f64 / target_trials as f64
        } else {
            0.0
        };
        let raw_false_alarm_rate = if non_target_trials > 0 {
            false_alarms as f64 / non_target_trials as f64
        } else {
            0.0
        };

        let (d_prime, criterion) =
            signal_detection_indices(hits, false_alarms, target_trials, non_target_trials);

        let accuracy = if total_trials > 0 {
            (hits + correct_rejections) as f64 / total_trials as f64
        } else {
            0.0
        };

        Self {
            total_trials,
            target_trials,
            non_target_trials,
            hits,
            misses,
            false_alarms,
            correct_rejections,
            hit_rate: raw_hit_rate,
            false_alarm_rate: raw_false_alarm_rate,
            accuracy,
            d_prime,
            criterion,
            mean_hit_rt_ms,
            median_hit_rt_ms,
            sd_hit_rt_ms,
            p10_hit_rt_ms,
            p90_hit_rt_ms,
            response_count,
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

fn signal_detection_indices(
    hits: u32,
    false_alarms: u32,
    target_trials: usize,
    non_target_trials: usize,
) -> (f64, f64) {
    let hit_trials = target_trials.max(1) as f64;
    let non_target_trials = non_target_trials.max(1) as f64;

    // Log-linear correction to avoid inf/NaN when rates hit 0 or 1.
    let adjusted_hit_rate = (hits as f64 + 0.5) / (hit_trials + 1.0);
    let adjusted_fa_rate = (false_alarms as f64 + 0.5) / (non_target_trials + 1.0);

    let z_hit = inverse_normal_cdf(adjusted_hit_rate.clamp(1e-6, 1.0 - 1e-6));
    let z_fa = inverse_normal_cdf(adjusted_fa_rate.clamp(1e-6, 1.0 - 1e-6));

    let d_prime = z_hit - z_fa;
    let criterion = -0.5 * (z_hit + z_fa);

    (d_prime, criterion)
}

/// Acklam's rational approximation implementation for the inverse CDF of the
/// standard normal distribution. Maximum error ~4.5e-4 across (0, 1).
fn inverse_normal_cdf(p: f64) -> f64 {
    // Coefficients for Acklam's approximation.
    const A: [f64; 6] = [
        -3.969_683_028_665_376e1,
        2.209_460_984_245_205e2,
        -2.759_285_104_469_687e2,
        1.383_577_518_672_69e2,
        -3.066_479_806_614_716e1,
        2.506_628_277_459_239,
    ];
    const B: [f64; 5] = [
        -5.447_609_879_822_406e1,
        1.615_858_368_580_409e2,
        -1.556_989_798_598_866e2,
        6.680_131_188_771_972e1,
        -1.328_068_155_288_572e1,
    ];
    const C: [f64; 6] = [
        -7.784_894_002_430_293e-3,
        -3.223_964_580_411_365e-1,
        -2.400_758_277_161_838,
        -2.549_732_539_343_734,
        4.374_664_141_464_968,
        2.938_163_982_698_783,
    ];
    const D: [f64; 4] = [
        7.784_695_709_041_462e-3,
        3.224_671_290_700_398e-1,
        2.445_134_137_142_996,
        3.754_408_661_907_416,
    ];

    const P_LOW: f64 = 0.02425;
    const P_HIGH: f64 = 1.0 - P_LOW;

    if p <= 0.0 {
        return f64::NEG_INFINITY;
    }

    if p >= 1.0 {
        return f64::INFINITY;
    }

    if p < P_LOW {
        let q = (-2.0 * p.ln()).sqrt();
        return (((((C[0] * q + C[1]) * q + C[2]) * q + C[3]) * q + C[4]) * q + C[5])
            / ((((D[0] * q + D[1]) * q + D[2]) * q + D[3]) * q + 1.0);
    }

    if p > P_HIGH {
        let q = (-2.0 * (1.0 - p).ln()).sqrt();
        return -(((((C[0] * q + C[1]) * q + C[2]) * q + C[3]) * q + C[4]) * q + C[5])
            / ((((D[0] * q + D[1]) * q + D[2]) * q + D[3]) * q + 1.0);
    }

    let q = p - 0.5;
    let r = q * q;
    (((((A[0] * r + A[1]) * r + A[2]) * r + A[3]) * r + A[4]) * r + A[5]) * q
        / (((((B[0] * r + B[1]) * r + B[2]) * r + B[3]) * r + B[4]) * r + 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tasks::nback::engine::{NBackTrial, TrialOutcome};

    #[test]
    fn metrics_count_hits_and_false_alarms() {
        let mut trials = Vec::new();

        trials.push(NBackTrial {
            index: 0,
            letter: 'A',
            is_target: false,
            is_lure: false,
            presented_at: None,
            response: None,
            outcome: TrialOutcome::CorrectRejection,
        });

        trials.push(NBackTrial {
            index: 1,
            letter: 'B',
            is_target: false,
            is_lure: false,
            presented_at: None,
            response: None,
            outcome: TrialOutcome::FalseAlarm { rt_ms: 420.0 },
        });

        trials.push(NBackTrial {
            index: 2,
            letter: 'C',
            is_target: true,
            is_lure: false,
            presented_at: None,
            response: None,
            outcome: TrialOutcome::Hit { rt_ms: 480.0 },
        });

        trials.push(NBackTrial {
            index: 3,
            letter: 'D',
            is_target: true,
            is_lure: false,
            presented_at: None,
            response: None,
            outcome: TrialOutcome::Miss,
        });

        let metrics = NBackMetrics::from_trials(&trials);
        assert_eq!(metrics.total_trials, 4);
        assert_eq!(metrics.hits, 1);
        assert_eq!(metrics.false_alarms, 1);
        assert_eq!(metrics.misses, 1);
        assert!(metrics.d_prime.is_finite());
        assert!(metrics.criterion.is_finite());
        assert_eq!(metrics.median_hit_rt_ms, 480.0);
    }
}
