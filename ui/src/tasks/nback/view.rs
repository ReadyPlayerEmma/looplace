use std::cell::RefCell;
use std::rc::Rc;

use dioxus::prelude::*;
use futures_channel::mpsc::{UnboundedReceiver, UnboundedSender};
use futures_util::StreamExt;

use crate::core::format;
use crate::core::qc::QualityFlags;
use crate::core::readiness::{self, Readiness};
use crate::core::{platform, storage, timing};

use super::engine::{
    AdvanceOutcome, EngineState, NBackEngine, ResponseOutcome, RunMode, TrialOutcome, TrialSchedule,
};
use super::metrics::NBackMetrics;

const FEEDBACK_HOLD_MS: u64 = 850;

// (Reverted) Use compile-time checked translation macro again.
// Removed temporary runtime helper that bypassed `fl!()` validation.

#[component]
pub fn NBackView() -> Element {
    // Subscribe to global language code so instructional section re-renders on locale switch.
    let _lang_code: Option<Signal<String>> = try_use_context::<Signal<String>>();
    let _lang_marker = _lang_code.as_ref().map(|s| s()).unwrap_or_default();

    let engine = use_signal(NBackEngine::default);
    let qc_flags = use_signal(QualityFlags::pristine);
    let practice_metrics = use_signal(|| Option::<NBackMetrics>::None);
    let last_metrics = use_signal(|| Option::<NBackMetrics>::None);
    let last_error = use_signal(|| Option::<String>::None);
    let feedback_state = use_signal(|| Option::<FeedbackState>::None);
    // Readiness (cooldown advisory) – compute once per mount; updates only after a run completes (page reload or future reactive trigger).
    let readiness_info = use_signal(|| match storage::load_summaries() {
        Ok(mut records) => {
            records.sort_by(|a, b| b.created_at.cmp(&a.created_at));
            let last = records.iter().find(|r| r.task == "nback2");
            readiness::evaluate("nback2", last)
        }
        Err(_) => readiness::evaluate("nback2", None),
    });

    let sender_slot: Rc<RefCell<Option<UnboundedSender<NBackEvent>>>> = Rc::new(RefCell::new(None));
    let sender_slot_for_loop = sender_slot.clone();

    let coroutine = {
        let engine_ref = engine;
        let qc_ref = qc_flags;
        let practice_ref = practice_metrics;
        let last_metrics_ref = last_metrics;
        let error_ref = last_error;
        let feedback_ref = feedback_state;

        use_coroutine(move |mut rx: UnboundedReceiver<NBackEvent>| {
            let sender_slot = sender_slot_for_loop.clone();
            let mut engine = engine_ref;
            let mut qc_flags = qc_ref;
            let mut practice_metrics = practice_ref;
            let mut last_metrics = last_metrics_ref;
            let mut last_error = error_ref;
            let mut feedback_signal = feedback_ref;

            async move {
                while let Some(event) = rx.next().await {
                    match event {
                        NBackEvent::StartPractice => {
                            practice_metrics.set(None);
                            last_error.set(None);
                            feedback_signal.set(None);
                            engine.with_mut(|eng| {
                                eng.config.response_window_ms =
                                    eng.config.stimulus_ms + eng.config.interstimulus_interval_ms;
                            });

                            if let Some(schedule) =
                                engine.with_mut(|eng| eng.start(RunMode::Practice))
                            {
                                queue_trial(sender_slot.clone(), schedule);
                            }
                        }
                        NBackEvent::StartMain => {
                            last_metrics.set(None);
                            last_error.set(None);
                            qc_flags.set(QualityFlags::pristine());
                            feedback_signal.set(None);
                            engine.with_mut(|eng| {
                                eng.config.response_window_ms =
                                    eng.config.stimulus_ms + eng.config.interstimulus_interval_ms;
                            });

                            if let Some(schedule) = engine.with_mut(|eng| eng.start(RunMode::Main))
                            {
                                queue_trial(sender_slot.clone(), schedule);
                            }
                        }
                        NBackEvent::Abort => {
                            engine.with_mut(|eng| eng.abort());
                        }
                        NBackEvent::StimulusReady {
                            run_id,
                            trial_index,
                            advance_wait_ms,
                        } => {
                            let should_schedule = engine.with_mut(|eng| {
                                if eng.run_id == run_id {
                                    eng.mark_stimulus_on(trial_index, timing::now())
                                } else {
                                    false
                                }
                            });

                            if should_schedule {
                                queue_advance(
                                    sender_slot.clone(),
                                    run_id,
                                    trial_index,
                                    advance_wait_ms,
                                );
                            }
                        }
                        NBackEvent::Respond { timestamp } => {
                            let response = engine.with_mut(|eng| eng.register_response(timestamp));

                            match response {
                                ResponseOutcome::Recorded(kind) => {
                                    let (message, tone) = match kind {
                                        super::engine::ResponseKind::Hit => {
                                            ("Match captured", FeedbackTone::Positive)
                                        }
                                        super::engine::ResponseKind::FalseAlarm => {
                                            ("Not a match", FeedbackTone::Negative)
                                        }
                                    };
                                    feedback_signal.set(Some(FeedbackState::new(message, tone)));
                                    let run_id = engine.with(|eng| eng.run_id);
                                    schedule_feedback_clear(
                                        sender_slot.clone(),
                                        run_id,
                                        FEEDBACK_HOLD_MS,
                                    );
                                }
                                ResponseOutcome::Ignored => {
                                    // Stale response outside active window.
                                }
                            }
                        }
                        NBackEvent::Advance {
                            run_id,
                            trial_index,
                        } => {
                            let (outcome, trial_snapshot) = engine.with_mut(|eng| {
                                if eng.run_id == run_id {
                                    let result = eng.advance(trial_index);
                                    let trial = eng
                                        .trials()
                                        .get(trial_index)
                                        .map(|trial| trial.outcome.clone());
                                    (result, trial)
                                } else {
                                    (AdvanceOutcome::Ignored, None)
                                }
                            });

                            if matches!(trial_snapshot, Some(TrialOutcome::Miss)) {
                                feedback_signal.set(Some(FeedbackState::new(
                                    "Missed match",
                                    FeedbackTone::Negative,
                                )));
                                schedule_feedback_clear(
                                    sender_slot.clone(),
                                    run_id,
                                    FEEDBACK_HOLD_MS,
                                );
                            }

                            match outcome {
                                AdvanceOutcome::Next(schedule) => {
                                    queue_trial(sender_slot.clone(), schedule);
                                }
                                AdvanceOutcome::Completed { mode } => {
                                    finalize_run(
                                        mode,
                                        &engine,
                                        qc_flags,
                                        practice_metrics,
                                        last_metrics,
                                        last_error,
                                        readiness_info,
                                    );
                                }
                                AdvanceOutcome::Ignored => {}
                            }
                        }
                        NBackEvent::FocusLost => {
                            qc_flags.with_mut(|flags| {
                                flags.log_focus_loss();
                                flags.log_visibility_blur();
                            });
                        }
                        NBackEvent::ClearFeedback { run_id } => {
                            let current_run = engine.with(|eng| eng.run_id);
                            if current_run == run_id {
                                feedback_signal.set(None);
                            }
                        }
                    }
                }
            }
        })
    };

    sender_slot.borrow_mut().replace(coroutine.tx());

    let send_event = {
        let coroutine_handle = coroutine;
        move |event: NBackEvent| coroutine_handle.send(event)
    };

    let respond_now = {
        let send_event_handle = send_event;
        move || {
            send_event_handle(NBackEvent::Respond {
                timestamp: timing::now(),
            });
        }
    };

    let engine_snapshot = engine();

    let (active_mode, current_letter) = match engine_snapshot.state {
        EngineState::Waiting { mode, .. } => (Some(mode), None),
        EngineState::StimulusActive { mode, trial_index } => {
            let letter = engine_snapshot
                .trials()
                .get(trial_index)
                .map(|trial| trial.letter);
            (Some(mode), letter)
        }
        _ => (None, None),
    };

    let is_running = active_mode.is_some();
    let total_trials = match active_mode.unwrap_or(RunMode::Main) {
        RunMode::Practice => engine_snapshot.config.practice_trials,
        RunMode::Main => engine_snapshot.config.total_trials,
    };
    let completed_trials = engine_snapshot
        .trials()
        .iter()
        .filter(|trial| trial.is_completed())
        .count();

    let last_practice = practice_metrics();
    let latest_metrics = last_metrics();
    let error_message = last_error();
    let error_message_cloned = error_message.clone();
    let feedback = feedback_state();

    let mode_label = active_mode
        .map(|mode| match mode {
            RunMode::Practice => crate::t!("nback-mode-practice"),
            RunMode::Main => crate::t!("nback-mode-main"),
        })
        .unwrap_or_default();

    rsx! {
        article { class: "task task-nback",

            if is_running {
                section { class: "task-card task-card--canvas task-nback__canvas",

                    button {
                        class: "button button--ghost button--compact task-canvas__cancel",
                        onclick: move |_| send_event(NBackEvent::Abort),
                        {crate::t!("common-cancel")}
                    }

                    if !mode_label.is_empty() {
                        div { class: "task-mode-badge", "{mode_label}" }
                    }

                    button {
                        r#type: "button",
                        class: "task-nback__hitbox",
                        aria_label: crate::t!("nback-aria-respond"),
                        autofocus: true,
                        onclick: move |_| respond_now(),
                        onkeydown: move |evt| {
                            let key = evt.key().to_string().to_lowercase();
                            if matches!(key.as_str(), " " | "space" | "spacebar" | "enter" | "j") {
                                evt.prevent_default();
                                respond_now();
                            }
                        },
                        onfocusout: move |_| send_event(NBackEvent::FocusLost),

                        div { class: "task-nback__glyph",
                            if let Some(letter) = current_letter {
                                "{letter}"
                            } else {
                                span { {crate::t!("nback-get-ready")} }
                            }
                        }

                        if let Some(feedback) = feedback.clone() {
                            div { class: format!("task-feedback {}", feedback.css_class()), "{feedback.message}" }
                        }
                    }

                    div { class: "task-progress task-progress--overlay",
                        span { {crate::t!("common-progress")} }
                        span { class: "task-progress__value", "{completed_trials}/{total_trials}" }
                    }
                }
            } else {
                // Readiness advisory banner (non-blocking)
                {
                    let r: Readiness = readiness_info();
                    rsx! {
                        section { class: format!("task-readiness {}", r.css_class()),
                            span { class: "task-readiness__status", "{r.status_label()}" }
                            span { class: "task-readiness__detail", "{r.detail_message()}" }
                        }
                    }
                }
                section { class: "task-card task-card--instructions task-nback__controls",
                    // Hidden i18n marker to force re-render of instruction copy when locale changes
                    div { style: "display:none", "{_lang_marker}" }
                    h3 { {crate::t!("nback-how-summary")} }
                    ul {
                        li { {crate::t!("nback-how-step-timing")} }
                        li { {crate::t!("nback-how-step-rule")} }
                        li { {crate::t!("nback-how-step-practice")} }
                        li { {crate::t!("nback-how-step-strategy")} }
                    }

                    div { class: "task-cta", style: "display:flex; gap:0.75rem; flex-wrap:wrap;",
                        button {
                            class: "button button--accent",
                            onclick: move |_| send_event(NBackEvent::StartPractice),
                            {crate::t!("nback-start-practice")}
                        }
                        button {
                            class: "button button--primary",
                            onclick: move |_| send_event(NBackEvent::StartMain),
                            {crate::t!("nback-start-main")}
                        }
                    }
                }

                if let Some(metrics) = last_practice {
                    section { class: "task-card task-card--subtle task-nback__practice-summary",
                        h3 { {crate::t!("nback-practice-recap")} }
                        p {
                            {crate::t!("nback-metric-hits")} " " {metrics.hits.to_string()} " / " {metrics.target_trials.to_string()}
                            " • " {crate::t!("nback-metric-false-alarms")} " " {metrics.false_alarms.to_string()}
                            " • " {crate::t!("nback-metric-accuracy")} " " {(metrics.accuracy * 100.0).round().to_string()} "%"
                        }
                        if metrics.hits > 0 {
                            p { {crate::t!("nback-practice-median-hit-rt")} " " {format::format_ms(metrics.median_hit_rt_ms)} }
                        }
                    }
                }

                if let Some(metrics) = latest_metrics {
                    section { class: "task-card task-nback__metrics",
                        h3 { {crate::t!("nback-last-session")} }
                        ul { class: "metrics-grid",
                            li { {crate::t!("nback-metric-hits")} ": " {metrics.hits.to_string()} " / " {metrics.target_trials.to_string()} }
                            li { {crate::t!("nback-metric-misses")} ": " {metrics.misses.to_string()} }
                            li { {crate::t!("nback-metric-false-alarms")} ": " {metrics.false_alarms.to_string()} }
                            li { {crate::t!("nback-metric-correct-rejections")} ": " {metrics.correct_rejections.to_string()} }
                            li { {crate::t!("nback-metric-accuracy")} ": " {(metrics.accuracy * 100.0).round().to_string()} "%" }
                            li { {crate::t!("nback-metric-dprime")} ": " {format!("{:.2}", metrics.d_prime).to_string()} }
                            li { {crate::t!("nback-metric-criterion")} ": " {format!("{:.2}", metrics.criterion).to_string()} }
                            li { {crate::t!("nback-metric-median-hit-rt")} ": " {format::format_ms(metrics.median_hit_rt_ms).to_string()} }
                            li { {crate::t!("nback-metric-mean-hit-rt")} ": " {format::format_ms(metrics.mean_hit_rt_ms).to_string()} }
                            li { {crate::t!("nback-metric-hit-rt-p10p90")} ": " {format::format_ms(metrics.p10_hit_rt_ms).to_string()} " / " {format::format_ms(metrics.p90_hit_rt_ms).to_string()} }
                        }
                    }
                } else {
                    section { class: "task-card task-nback__metrics task-metrics--placeholder",
                        p { {crate::t!("nback-metrics-placeholder")} }
                    }
                }

                if let Some(err) = error_message_cloned {
                    div { class: "task-error", {crate::t!("nback-error-generic", message = err.clone())} }
                }
            }
        }
    }
}

fn finalize_run(
    mode: RunMode,
    engine: &Signal<NBackEngine>,
    mut qc_flags: Signal<QualityFlags>,
    mut practice_metrics: Signal<Option<NBackMetrics>>,
    mut last_metrics: Signal<Option<NBackMetrics>>,
    mut last_error: Signal<Option<String>>,
    mut readiness_info: Signal<Readiness>,
) {
    match mode {
        RunMode::Practice => {
            if let Some(metrics) = engine.with(|eng| eng.practice_metrics()) {
                practice_metrics.set(Some(metrics));
            }
        }
        RunMode::Main => {
            if let Some(metrics) = engine.with(|eng| eng.main_metrics()) {
                qc_flags.with_mut(|flags| flags.mark_min_trials(true));
                let qc_snapshot = qc_flags();
                match serde_json::to_value(&metrics) {
                    Ok(metrics_json) => {
                        let record =
                            storage::SummaryRecord::new("nback2", metrics_json, qc_snapshot);
                        if let Err(err) = storage::append_summary(&record) {
                            last_error.set(Some(format!("Failed to persist summary: {err}")));
                        } else {
                            last_error.set(None);
                        }
                        last_metrics.set(Some(metrics));

                        // Refresh readiness advisory immediately so the cooldown banner updates
                        if let Ok(mut records) = storage::load_summaries() {
                            records.sort_by(|a, b| b.created_at.cmp(&a.created_at));
                            let last = records.iter().find(|r| r.task == "nback2");
                            let updated = readiness::evaluate("nback2", last);
                            readiness_info.set(updated);
                        }
                    }
                    Err(err) => {
                        last_error.set(Some(format!("Failed to serialise metrics: {err}")));
                    }
                }
            }
        }
    }
}

fn queue_trial(
    sender_slot: Rc<RefCell<Option<UnboundedSender<NBackEvent>>>>,
    schedule: TrialSchedule,
) {
    queue_stimulus(sender_slot, schedule.stimulus, schedule.advance.wait_ms);
}

fn queue_stimulus(
    sender_slot: Rc<RefCell<Option<UnboundedSender<NBackEvent>>>>,
    schedule: super::engine::ScheduledStimulus,
    advance_wait_ms: u64,
) {
    if let Some(sender) = sender_slot.borrow().as_ref().cloned() {
        platform::spawn_future(async move {
            timing::sleep_ms(schedule.wait_ms).await;
            let _ = sender.unbounded_send(NBackEvent::StimulusReady {
                run_id: schedule.run_id,
                trial_index: schedule.trial_index,
                advance_wait_ms,
            });
        });
    }
}

fn queue_advance(
    sender_slot: Rc<RefCell<Option<UnboundedSender<NBackEvent>>>>,
    run_id: u64,
    trial_index: usize,
    wait_ms: u64,
) {
    if let Some(sender) = sender_slot.borrow().as_ref().cloned() {
        platform::spawn_future(async move {
            timing::sleep_ms(wait_ms).await;
            let _ = sender.unbounded_send(NBackEvent::Advance {
                run_id,
                trial_index,
            });
        });
    }
}

fn schedule_feedback_clear(
    sender_slot: Rc<RefCell<Option<UnboundedSender<NBackEvent>>>>,
    run_id: u64,
    wait_ms: u64,
) {
    if let Some(sender) = sender_slot.borrow().as_ref().cloned() {
        platform::spawn_future(async move {
            timing::sleep_ms(wait_ms).await;
            let _ = sender.unbounded_send(NBackEvent::ClearFeedback { run_id });
        });
    }
}

#[derive(Debug, Clone)]
enum NBackEvent {
    StartPractice,
    StartMain,
    Abort,
    StimulusReady {
        run_id: u64,
        trial_index: usize,
        advance_wait_ms: u64,
    },
    Advance {
        run_id: u64,
        trial_index: usize,
    },
    Respond {
        timestamp: crate::core::timing::InstantStamp,
    },
    FocusLost,
    ClearFeedback {
        run_id: u64,
    },
}

#[derive(Debug, Clone)]
struct FeedbackState {
    message: String,
    tone: FeedbackTone,
}

impl FeedbackState {
    fn new<M: Into<String>>(message: M, tone: FeedbackTone) -> Self {
        Self {
            message: message.into(),
            tone,
        }
    }

    fn css_class(&self) -> &'static str {
        match self.tone {
            FeedbackTone::Positive => "task-feedback--positive",
            FeedbackTone::Negative => "task-feedback--negative",
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum FeedbackTone {
    Positive,
    Negative,
}
