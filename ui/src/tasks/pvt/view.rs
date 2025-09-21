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

const TICK_INTERVAL_MS: u64 = 33;
const FEEDBACK_HOLD_MS: u64 = 900;

#[component]
pub fn PvtView() -> Element {
    let engine = use_signal(PvtEngine::default);
    let qc_flags = use_signal(QualityFlags::pristine);
    let last_metrics = use_signal(|| Option::<PvtMetrics>::None);
    let indicator_text = use_signal(|| "READY".to_string());
    let last_error = use_signal(|| Option::<String>::None);

    let sender_slot: Rc<RefCell<Option<UnboundedSender<PvtEvent>>>> = Rc::new(RefCell::new(None));
    let sender_slot_for_loop = sender_slot.clone();

    let coroutine = {
        let engine_ref = engine.clone();
        let qc_ref = qc_flags.clone();
        let metrics_ref = last_metrics.clone();
        let indicator_ref = indicator_text.clone();
        let error_ref = last_error.clone();

        use_coroutine(move |mut rx: UnboundedReceiver<PvtEvent>| {
            let sender_slot = sender_slot_for_loop.clone();
            let mut engine_signal = engine_ref.clone();
            let mut qc_signal = qc_ref.clone();
            let mut metrics_signal = metrics_ref.clone();
            let mut indicator_signal = indicator_ref.clone();
            let mut error_signal = error_ref.clone();

            async move {
                while let Some(event) = rx.next().await {
                    match event {
                        PvtEvent::Start => {
                            error_signal.set(None);
                            metrics_signal.set(None);
                            qc_signal.set(QualityFlags::pristine());
                            indicator_signal.set("WAIT".to_string());

                            let scheduled = engine_signal.with_mut(|eng| eng.start());
                            if let Some(schedule) = scheduled {
                                queue_stimulus(sender_slot.clone(), schedule);
                            } else {
                                indicator_signal.set("RUN".to_string());
                            }
                        }
                        PvtEvent::Abort => {
                            engine_signal.with_mut(|eng| eng.abort());
                            indicator_signal.set("ABORT".to_string());
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
                                    indicator_signal.set("000".to_string());
                                    Some(eng.config.max_response_ms)
                                } else {
                                    None
                                }
                            });

                            if let Some(window_ms) = maybe_window {
                                queue_timeout(sender_slot.clone(), run_id, trial_index, window_ms);
                                schedule_tick(sender_slot.clone(), run_id);
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

                                    match last_outcome {
                                        Some(TrialOutcome::Reaction { rt_ms }) => {
                                            indicator_signal.set(format!(
                                                "{:03}",
                                                rt_ms.round().clamp(0.0, 999.0) as u32
                                            ));
                                            let run_id = engine_signal.with(|eng| eng.run_id);
                                            schedule_indicator_reset(sender_slot.clone(), run_id);
                                        }
                                        Some(TrialOutcome::FalseStart) => {
                                            indicator_signal.set("FS".to_string());
                                            let run_id = engine_signal.with(|eng| eng.run_id);
                                            schedule_indicator_reset(sender_slot.clone(), run_id);
                                        }
                                        Some(TrialOutcome::Lapse) => {
                                            indicator_signal.set("LAP".to_string());
                                            let run_id = engine_signal.with(|eng| eng.run_id);
                                            schedule_indicator_reset(sender_slot.clone(), run_id);
                                        }
                                        _ => {}
                                    }
                                    queue_stimulus(sender_slot.clone(), schedule);
                                }
                                ResponseOutcome::RunCompleted => {
                                    finalize_run(
                                        &engine_signal,
                                        qc_signal.clone(),
                                        metrics_signal.clone(),
                                        indicator_signal.clone(),
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
                                    indicator_signal.set("LAP".to_string());
                                    let run_id = engine_signal.with(|eng| eng.run_id);
                                    schedule_indicator_reset(sender_slot.clone(), run_id);
                                    queue_stimulus(sender_slot.clone(), schedule);
                                }
                                ResponseOutcome::RunCompleted => {
                                    indicator_signal.set("LAP".to_string());
                                    finalize_run(
                                        &engine_signal,
                                        qc_signal.clone(),
                                        metrics_signal.clone(),
                                        indicator_signal.clone(),
                                        error_signal.clone(),
                                    );
                                }
                                ResponseOutcome::Ignored => {}
                            }
                        }
                        PvtEvent::Tick { run_id } => {
                            let continue_loop = engine_signal.with(|eng| {
                                if eng.run_id != run_id {
                                    return false;
                                }

                                if let EngineState::StimulusActive { trial_index } = eng.state {
                                    if let Some(trial) = eng.trials.get(trial_index) {
                                        if let Some(onset) = trial.stimulus_onset {
                                            let elapsed = timing::duration_ms(onset, timing::now());
                                            let clamped = elapsed.clamp(0.0, 999.0);
                                            indicator_signal
                                                .set(format!("{:03}", clamped.round() as u32));
                                            return true;
                                        }
                                    }
                                }

                                false
                            });

                            if continue_loop {
                                schedule_tick(sender_slot.clone(), run_id);
                            }
                        }
                        PvtEvent::ResetIndicator { run_id } => {
                            let should_reset = engine_signal.with(|eng| {
                                eng.run_id == run_id
                                    && matches!(eng.state, EngineState::Waiting { .. })
                            });

                            if should_reset {
                                indicator_signal.set("WAIT".to_string());
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

    let guidance_text = match engine_snapshot.state {
        EngineState::Idle => {
            "Press start, then wait for the milliseconds counter to appear.".to_string()
        }
        EngineState::Waiting { .. } => "Hold steady… the counter will appear soon.".to_string(),
        EngineState::StimulusActive { .. } => {
            "Tap or press space the moment the counter appears.".to_string()
        }
        EngineState::Completed => "Session complete. Start again when ready.".to_string(),
        EngineState::Aborted => "Run cancelled. Start to retry.".to_string(),
    };

    rsx! {
        article { class: "task task-pvt",
            style: "display:flex; flex-direction:column; gap:2rem;",

            if is_running {
                div {
                    class: "task-pvt__canvas",
                    style: "position:relative; min-height:60vh; display:flex; flex-direction:column; align-items:center; justify-content:center; background:#05060a; color:#f7f7f7; border-radius:16px;",

                    button {
                        class: "task-pvt__cancel",
                        style: "position:absolute; top:1.5rem; left:1.5rem; background:transparent; color:#f7f7f7; border:1px solid rgba(247,247,247,0.4); padding:0.5rem 1rem; border-radius:999px; font-size:0.9rem; z-index:2; pointer-events:auto;",
                        onclick: move |_| send_event(PvtEvent::Abort),
                        "Cancel"
                    }

                    button {
                        r#type: "button",
                        class: "task-pvt__hitbox",
                        aria_label: "PVT reaction target",
                        autofocus: true,
                        style: "position:absolute; inset:0; display:flex; align-items:center; justify-content:center; background:transparent; border:none; color:inherit; cursor:pointer; z-index:1;",
                        onfocusout: move |_| send_event(PvtEvent::FocusLost),
                        onclick: move |_| respond_now(),
                        onkeydown: move |evt| {
                            let key = evt.key().to_string().to_lowercase();
                            if key == " " || key == "space" || key == "spacebar" || key == "enter" {
                                evt.prevent_default();
                                respond_now();
                            }
                        },

                        div {
                            class: "task-pvt__indicator",
                            style: "font-size:6rem; letter-spacing:0.2rem; font-family:'JetBrains Mono', 'Fira Mono', monospace;",
                            {indicator_text()}
                        }
                    }

                    div {
                        class: "task-pvt__message",
                        style: "position:absolute; bottom:2rem; text-align:center; font-size:1rem; color:rgba(247,247,247,0.8);",
                        "{guidance_text}"
                    }

                    div {
                        class: "task-pvt__progress",
                        style: "position:absolute; top:1.5rem; right:1.5rem; font-size:0.9rem; letter-spacing:0.1rem; color:rgba(247,247,247,0.7);",
                        "{trial_progress}/{total_target}"
                    }
                }
            } else {
                section {
                    class: "task-pvt__prelude",
                    style: "display:flex; flex-direction:column; gap:1rem;",

                    p {
                        "Wait until the millisecond counter appears in the centre, then tap or press space immediately."
                    }
                    p {
                        "This implementation mirrors the canonical PVT-192 stimulus: 2–10 s uniform jitter, false starts add delay, lapses ≥500 ms are flagged."
                    }

                    div { style: "display:flex; gap:1rem; align-items:center;",
                        button {
                            r#type: "button",
                            class: "task-pvt__start",
                            style: "padding:0.75rem 1.75rem; font-size:1rem; border-radius:999px; border:none; background:#f05a7e; color:#fff; font-weight:600;",
                            onclick: move |_| send_event(PvtEvent::Start),
                            "Start"
                        }
                        span { style: "color:#666;", "{guidance_text}" }
                    }
                }
            }

            if !is_running {
                if let Some(metrics) = latest_metrics {
                    div { class: "task-pvt__metrics",
                        style: "display:flex; flex-direction:column; gap:0.5rem;",
                        h3 { "Last session" }
                        ul {
                            style: "display:grid; grid-template-columns:repeat(auto-fit,minmax(180px,1fr)); gap:0.5rem; list-style:none; padding:0;",
                            li { "Median RT: {format::format_ms(metrics.median_rt_ms)}" }
                            li { "Mean RT: {format::format_ms(metrics.mean_rt_ms)}" }
                            li { "SD RT: {format::format_ms(metrics.sd_rt_ms)}" }
                            li { "P10: {format::format_ms(metrics.p10_rt_ms)}" }
                            li { "P90: {format::format_ms(metrics.p90_rt_ms)}" }
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
                        style: "padding:1rem 1.5rem; border-radius:12px; background:rgba(255,255,255,0.04); color:#666;",
                        p { "Metrics will appear after the first completed run." }
                    }
                }

                if let Some(err) = error_message {
                    div { class: "task-pvt__error", style: "color:#c21d4a; font-weight:600;", "⚠️ {err}" }
                }
            }
        }
    }
}

fn finalize_run(
    engine: &Signal<PvtEngine>,
    mut qc_flags: Signal<QualityFlags>,
    mut last_metrics: Signal<Option<PvtMetrics>>,
    mut indicator_text: Signal<String>,
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
                    indicator_text.set("DONE".to_string());
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

fn schedule_tick(sender_slot: Rc<RefCell<Option<UnboundedSender<PvtEvent>>>>, run_id: u64) {
    if let Some(sender) = sender_slot.borrow().as_ref().cloned() {
        platform::spawn_future(async move {
            timing::sleep_ms(TICK_INTERVAL_MS).await;
            let _ = sender.unbounded_send(PvtEvent::Tick { run_id });
        });
    }
}

fn schedule_indicator_reset(
    sender_slot: Rc<RefCell<Option<UnboundedSender<PvtEvent>>>>,
    run_id: u64,
) {
    if let Some(sender) = sender_slot.borrow().as_ref().cloned() {
        platform::spawn_future(async move {
            timing::sleep_ms(FEEDBACK_HOLD_MS).await;
            let _ = sender.unbounded_send(PvtEvent::ResetIndicator { run_id });
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
    Tick { run_id: u64 },
    ResetIndicator { run_id: u64 },
    FocusLost,
}
