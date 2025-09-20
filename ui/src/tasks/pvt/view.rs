use std::cell::RefCell;
use std::rc::Rc;

use dioxus::prelude::*;
use futures_channel::mpsc::UnboundedSender;
use futures_util::StreamExt;

use crate::core::qc::QualityFlags;
use crate::core::timing::InstantStamp;
use crate::core::{format, platform, storage, timing};

use super::engine::{EngineState, PvtEngine, ResponseOutcome, ScheduledStimulus, TrialOutcome};
use super::metrics::PvtMetrics;

#[component]
pub fn PvtView() -> Element {
    let engine = use_signal(PvtEngine::default);
    let qc_flags = use_signal(QualityFlags::pristine);
    let last_metrics = use_signal(|| Option::<PvtMetrics>::None);
    let status_line = use_signal(|| "Press start to begin.".to_string());
    let last_error = use_signal(|| Option::<String>::None);

    let sender_slot: Rc<RefCell<Option<UnboundedSender<PvtEvent>>>> = Rc::new(RefCell::new(None));
    let sender_slot_for_loop = sender_slot.clone();

    let coroutine = {
        let engine_ref = engine.clone();
        let qc_ref = qc_flags.clone();
        let metrics_ref = last_metrics.clone();
        let status_ref = status_line.clone();
        let error_ref = last_error.clone();

        use_coroutine(move |mut rx: UnboundedReceiver<PvtEvent>| {
            let sender_slot = sender_slot_for_loop.clone();
            let mut engine_signal = engine_ref.clone();
            let mut qc_signal = qc_ref.clone();
            let mut metrics_signal = metrics_ref.clone();
            let mut status_signal = status_ref.clone();
            let mut error_signal = error_ref.clone();

            async move {
                while let Some(event) = rx.next().await {
                    match event {
                        PvtEvent::Start => {
                            error_signal.set(None);
                            metrics_signal.set(None);
                            qc_signal.set(QualityFlags::pristine());

                            let scheduled = engine_signal.with_mut(|eng| eng.start());
                            if let Some(schedule) = scheduled {
                                status_signal.set("Get ready for the first cue.".to_string());
                                queue_stimulus(sender_slot.clone(), schedule);
                            } else {
                                status_signal.set("Run already in progress.".to_string());
                            }
                        }
                        PvtEvent::Abort => {
                            engine_signal.with_mut(|eng| eng.abort());
                            status_signal.set("Run aborted.".to_string());
                        }
                        PvtEvent::StimulusReady {
                            run_id,
                            trial_index,
                        } => {
                            let maybe_window = engine_signal.with_mut(|eng| {
                                if eng.run_id != run_id {
                                    return None;
                                }
                                if eng.mark_stimulus_on(trial_index, timing::now()) {
                                    status_signal.set("GO! Respond now.".to_string());
                                    Some(eng.config.max_response_ms)
                                } else {
                                    None
                                }
                            });

                            if let Some(window_ms) = maybe_window {
                                queue_timeout(sender_slot.clone(), run_id, trial_index, window_ms);
                            }
                        }
                        PvtEvent::Respond { timestamp } => {
                            let outcome =
                                engine_signal.with_mut(|eng| eng.register_response(timestamp));

                            match outcome {
                                ResponseOutcome::NextScheduled(schedule) => {
                                    let last_outcome = engine_signal.with(|eng| {
                                        eng.trials
                                            .iter()
                                            .rev()
                                            .find(|trial| trial.is_completed())
                                            .map(|trial| trial.outcome.clone())
                                    });

                                    if let Some(TrialOutcome::Reaction { rt_ms }) = last_outcome {
                                        status_signal.set(format!(
                                            "Reaction captured: {}",
                                            format::format_ms(rt_ms)
                                        ));
                                    } else {
                                        status_signal.set("False start registered.".to_string());
                                    }

                                    queue_stimulus(sender_slot.clone(), schedule);
                                }
                                ResponseOutcome::RunCompleted => {
                                    finalize_run(
                                        &engine_signal,
                                        qc_signal.clone(),
                                        metrics_signal.clone(),
                                        status_signal.clone(),
                                        error_signal.clone(),
                                    );
                                }
                                ResponseOutcome::Ignored => {}
                            }
                        }
                        PvtEvent::Timeout {
                            run_id,
                            trial_index,
                        } => {
                            let outcome = engine_signal.with_mut(|eng| {
                                if eng.run_id != run_id {
                                    ResponseOutcome::Ignored
                                } else {
                                    eng.register_timeout(trial_index)
                                }
                            });

                            match outcome {
                                ResponseOutcome::NextScheduled(schedule) => {
                                    status_signal
                                        .set("Lapse recorded. Next cue pending.".to_string());
                                    queue_stimulus(sender_slot.clone(), schedule);
                                }
                                ResponseOutcome::RunCompleted => {
                                    finalize_run(
                                        &engine_signal,
                                        qc_signal.clone(),
                                        metrics_signal.clone(),
                                        status_signal.clone(),
                                        error_signal.clone(),
                                    );
                                }
                                ResponseOutcome::Ignored => {}
                            }
                        }
                        PvtEvent::FocusLost => {
                            qc_signal.with_mut(|flags| {
                                flags.log_focus_loss();
                                flags.log_visibility_blur();
                            });
                        }
                    }
                }
            }
        })
    };

    sender_slot.borrow_mut().replace(coroutine.tx());

    let send_event = {
        let coroutine = coroutine.clone();
        move |event: PvtEvent| {
            coroutine.send(event);
        }
    };

    let respond_now = {
        let send_event = send_event.clone();
        move || {
            send_event(PvtEvent::Respond {
                timestamp: timing::now(),
            });
        }
    };

    let engine_snapshot = engine();
    let is_running = matches!(
        engine_snapshot.state,
        EngineState::Waiting { .. } | EngineState::StimulusActive { .. }
    );
    let trial_progress = engine_snapshot
        .trials
        .iter()
        .filter(|trial| trial.is_completed())
        .count();
    let total_target = engine_snapshot.config.target_trials;

    let latest_metrics = last_metrics();
    let error_message = last_error();

    rsx! {
        article { class: "task task-pvt",
            div { class: "task-pvt__header",
                h2 { "Psychomotor Vigilance Task" }
                p { "Focus on the panel below. Press the space bar or tap as soon as the stimulus appears." }
            }

            div { class: "task-pvt__controls",
                button {
                    r#type: "button",
                    class: "task-pvt__start",
                    disabled: is_running,
                    onclick: move |_| send_event(PvtEvent::Start),
                    "Start"
                }
                button {
                    r#type: "button",
                    class: "task-pvt__abort",
                    disabled: !is_running,
                    onclick: move |_| send_event(PvtEvent::Abort),
                    "Abort"
                }
                span { class: "task-pvt__progress",
                    "Trials: {trial_progress}/{total_target}"
                }
            }

            div {
                class: "task-pvt__target",
                tabindex: 0,
                role: "button",
                aria_label: "PVT reaction target",
                onfocusout: move |_| send_event(PvtEvent::FocusLost),
                onclick: move |_| respond_now(),
                onkeydown: move |evt| {
                    let key = evt.key().to_string().to_lowercase();
                    if key == " " || key == "space" || key == "spacebar" || key == "enter" {
                        evt.prevent_default();
                        respond_now();
                    }
                },
                {status_line()}
            }

            if let Some(metrics) = latest_metrics {
                div { class: "task-pvt__metrics",
                    h3 { "Session metrics" }
                    ul {
                        li { "Median RT: {format::format_ms(metrics.median_rt_ms)}" }
                        li { "Mean RT: {format::format_ms(metrics.mean_rt_ms)}" }
                        li { "SD RT: {format::format_ms(metrics.sd_rt_ms)}" }
                        li { "10th percentile: {format::format_ms(metrics.p10_rt_ms)}" }
                        li { "90th percentile: {format::format_ms(metrics.p90_rt_ms)}" }
                        li { "Lapses ≥500 ms: {metrics.lapses_ge_500ms}" }
                        li { "Minor lapses 355–499 ms: {metrics.minor_lapses_355_499ms}" }
                        li { "False starts: {metrics.false_starts}" }
                        li { "Slope: {format::format_slope(metrics.time_on_task_slope_ms_per_min)}" }
                        li {
                            "Min trials met: "
                            if metrics.meets_min_trial_requirement { "Yes" } else { "No" }
                        }
                    }
                }
            } else {
                div { class: "task-pvt__metrics task-pvt__metrics--placeholder",
                    p { "Metrics will appear after the current run finishes." }
                }
            }

            if let Some(err) = error_message {
                div { class: "task-pvt__error", "⚠️ {err}" }
            }
        }
    }
}

fn finalize_run(
    engine: &Signal<PvtEngine>,
    mut qc_flags: Signal<QualityFlags>,
    mut last_metrics: Signal<Option<PvtMetrics>>,
    mut status_line: Signal<String>,
    mut last_error: Signal<Option<String>>,
) {
    if let Some(metrics) = engine.with(|eng| eng.metrics()) {
        qc_flags.with_mut(|flags| flags.mark_min_trials(metrics.meets_min_trial_requirement));
        let qc_snapshot = qc_flags();

        match serde_json::to_value(&metrics) {
            Ok(metrics_json) => {
                let record = storage::SummaryRecord::new("pvt", metrics_json, qc_snapshot.clone());
                if let Err(err) = storage::append_summary(&record) {
                    last_error.set(Some(format!("Failed to persist summary: {err}")));
                } else {
                    last_error.set(None);
                    status_line.set("Session complete. Summary saved.".to_string());
                }
            }
            Err(err) => {
                last_error.set(Some(format!("Failed to serialise metrics: {err}")));
            }
        }

        last_metrics.set(Some(metrics));
    }
}

fn queue_stimulus(
    sender_slot: Rc<RefCell<Option<UnboundedSender<PvtEvent>>>>,
    schedule: ScheduledStimulus,
) {
    if let Some(sender) = sender_slot.borrow().as_ref().cloned() {
        platform::spawn_future(async move {
            timing::sleep_ms(schedule.wait_ms).await;
            let _ = sender.unbounded_send(PvtEvent::StimulusReady {
                run_id: schedule.run_id,
                trial_index: schedule.trial_index,
            });
        });
    }
}

fn queue_timeout(
    sender_slot: Rc<RefCell<Option<UnboundedSender<PvtEvent>>>>,
    run_id: u64,
    trial_index: usize,
    timeout_ms: u64,
) {
    if let Some(sender) = sender_slot.borrow().as_ref().cloned() {
        platform::spawn_future(async move {
            timing::sleep_ms(timeout_ms).await;
            let _ = sender.unbounded_send(PvtEvent::Timeout {
                run_id,
                trial_index,
            });
        });
    }
}

#[derive(Debug, Clone)]
enum PvtEvent {
    Start,
    Abort,
    StimulusReady { run_id: u64, trial_index: usize },
    Timeout { run_id: u64, trial_index: usize },
    Respond { timestamp: InstantStamp },
    FocusLost,
}
