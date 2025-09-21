use std::cell::RefCell;
use std::rc::Rc;

use dioxus::prelude::*;
use futures_channel::mpsc::{UnboundedReceiver, UnboundedSender};
use futures_util::StreamExt;

use crate::core::format;
use crate::core::qc::QualityFlags;
use crate::core::{platform, storage, timing};

use super::engine::{
    AdvanceOutcome, EngineState, NBackEngine, ResponseOutcome, RunMode, TrialOutcome, TrialSchedule,
};
use super::metrics::NBackMetrics;

const PRACTICE_LABEL: &str = "Practice";
const MAIN_LABEL: &str = "Main session";
const FEEDBACK_HOLD_MS: u64 = 850;

#[component]
pub fn NBackView() -> Element {
    let engine = use_signal(NBackEngine::default);
    let qc_flags = use_signal(QualityFlags::pristine);
    let practice_metrics = use_signal(|| Option::<NBackMetrics>::None);
    let last_metrics = use_signal(|| Option::<NBackMetrics>::None);
    let last_error = use_signal(|| Option::<String>::None);
    let feedback_state = use_signal(|| Option::<FeedbackState>::None);

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
    let feedback = feedback_state();

    let mode_label = active_mode
        .map(|mode| match mode {
            RunMode::Practice => PRACTICE_LABEL,
            RunMode::Main => MAIN_LABEL,
        })
        .unwrap_or("");

    rsx! {
        article { class: "task task-nback",
            style: "display:flex; flex-direction:column; gap:2rem;",

            section { class: "task-nback__intro",
                style: "display:flex; flex-direction:column; gap:0.5rem;",
                details {
                    class: "task-nback__instructions",
                    style: "background:rgba(255,255,255,0.04); border-radius:12px; padding:1rem 1.25rem;",
                    summary { "How the task works" }
                    ul { style: "padding-left:1.1rem; display:flex; flex-direction:column; gap:0.35rem;",
                        li { "Each letter displays for 0.5 s, followed by 2.5 s of blank interval." }
                        li { "Press space (or tap the pad) whenever the letter matches the one from two trials ago." }
                        li { "Practice block lasts ~35 seconds; main run is about 3 minutes." }
                        li { "Focus on accuracy first, then speed. False alarms tax d′ just like misses." }
                    }
                }
            }

            if is_running {
                section { class: "task-nback__canvas",
                    style: "position:relative; min-height:55vh; display:flex; flex-direction:column; align-items:center; justify-content:center; background:#10121a; color:#f7f7f7; border-radius:16px; gap:1.5rem; padding:2.5rem 1.5rem;",

                    button {
                        class: "task-nback__cancel",
                        style: "position:absolute; top:1.5rem; left:1.5rem; background:transparent; color:#f7f7f7; border:1px solid rgba(247,247,247,0.4); padding:0.45rem 1.1rem; border-radius:999px; font-size:0.9rem;",
                        onclick: move |_| send_event(NBackEvent::Abort),
                        "Cancel"
                    }

                    div {
                        class: "task-nback__mode",
                        style: "position:absolute; top:1.5rem; right:1.5rem; text-transform:uppercase; letter-spacing:0.12rem; font-size:0.8rem; color:rgba(247,247,247,0.7);",
                        "{mode_label}"
                    }

                    button {
                        r#type: "button",
                        class: "task-nback__hitbox",
                        aria_label: "Respond to current letter",
                        autofocus: true,
                        style: "display:flex; align-items:center; justify-content:center; width:100%; height:100%; max-width:420px; aspect-ratio:1/1; background:rgba(247,247,247,0.04); border:1px solid rgba(247,247,247,0.2); border-radius:20px; color:inherit; cursor:pointer;",
                        onclick: move |_| respond_now(),
                        onkeydown: move |evt| {
                            let key = evt.key().to_string().to_lowercase();
                            if matches!(key.as_str(), " " | "space" | "spacebar" | "enter" | "j") {
                                evt.prevent_default();
                                respond_now();
                            }
                        },
                        onfocusout: move |_| send_event(NBackEvent::FocusLost),

                        div {
                            class: "task-nback__glyph",
                            style: "font-size:8rem; font-weight:700; letter-spacing:0.2rem; font-family:'Poppins', 'Inter', sans-serif; color:#f7f7f7;",
                            if let Some(letter) = current_letter {
                                "{letter}"
                            } else {
                                span { style: "font-size:1.1rem; letter-spacing:0.08rem; color:rgba(247,247,247,0.6);",
                                    "Get ready"
                                }
                            }
                        }

                        if let Some(feedback) = feedback.clone() {
                            div {
                                class: "task-nback__feedback",
                                style: format!(
                                    "position:absolute; bottom:2rem; width:80%; max-width:320px; text-align:center; padding:0.6rem 1rem; border-radius:999px; font-weight:600; letter-spacing:0.04rem; background:{}; color:{}; box-shadow:0 8px 24px rgba(0,0,0,0.25);",
                                    feedback.background(),
                                    feedback.foreground()
                                ),
                                "{feedback.message}"
                            }
                        }
                    }

                    div {
                        class: "task-nback__progress",
                        style: "display:flex; flex-direction:column; align-items:center; gap:0.35rem; color:rgba(247,247,247,0.75);",
                        span { style: "font-size:0.85rem; text-transform:uppercase; letter-spacing:0.18rem;", "Progress" }
                        span { style: "font-size:1.1rem; font-weight:600; letter-spacing:0.12rem;", "{completed_trials}/{total_trials}" }
                    }
                }
            } else {
                section { class: "task-nback__controls",
                    style: "display:flex; flex-direction:column; gap:1.5rem;",

                    div {
                        class: "task-nback__cta",
                        style: "display:flex; flex-wrap:wrap; gap:0.75rem;",
                        button {
                            class: "task-nback__start button--primary",
                            style: "padding:0.75rem 1.5rem; border-radius:999px; border:none; background:#1f68ff; color:white; font-size:1rem; font-weight:600; cursor:pointer;",
                            onclick: move |_| send_event(NBackEvent::StartPractice),
                            "Start practice"
                        }
                        button {
                            class: "task-nback__start-main",
                            style: "padding:0.75rem 1.5rem; border-radius:999px; border:none; background:#f05a7e; color:#fff; font-size:1rem; font-weight:600; cursor:pointer;",
                            onclick: move |_| send_event(NBackEvent::StartMain),
                            "Start main session"
                        }
                    }

                    if let Some(metrics) = last_practice {
                        section { class: "task-nback__practice-summary",
                            style: "display:flex; flex-direction:column; gap:0.6rem; padding:1rem 1.25rem; border-radius:12px; background:rgba(0,0,0,0.035);",
                            h3 { style: "margin:0; font-size:1rem; text-transform:uppercase; letter-spacing:0.12rem; color:#4a5da9;", "Practice recap" }
                            p {
                                style: "margin:0; color:#4b4f58;",
                                "Hits {metrics.hits} / {metrics.target_trials} • False alarms {metrics.false_alarms} • Accuracy {(metrics.accuracy * 100.0).round()}%"
                            }
                            if metrics.hits > 0 {
                                p { style: "margin:0; color:#4b4f58;",
                                    "Median hit RT {format::format_ms(metrics.median_hit_rt_ms)}"
                                }
                            }
                        }
                    }

                    if let Some(metrics) = latest_metrics {
                        section { class: "task-nback__metrics",
                            style: "display:flex; flex-direction:column; gap:0.75rem;",
                            h3 { style: "margin:0; font-size:1.05rem;", "Last main session" }
                            ul {
                                style: "display:grid; grid-template-columns:repeat(auto-fit,minmax(210px,1fr)); gap:0.45rem; list-style:none; padding:0; margin:0;",
                                li { "Hits: {metrics.hits} / {metrics.target_trials}" }
                                li { "Misses: {metrics.misses}" }
                                li { "False alarms: {metrics.false_alarms}" }
                                li { "Correct rejections: {metrics.correct_rejections}" }
                                li { "Accuracy: {(metrics.accuracy * 100.0).round()}%" }
                                li { "d′: {metrics.d_prime:.2}" }
                                li { "Criterion: {metrics.criterion:.2}" }
                                li { "Median hit RT: {format::format_ms(metrics.median_hit_rt_ms)}" }
                                li { "Mean hit RT: {format::format_ms(metrics.mean_hit_rt_ms)}" }
                                li { "Hit RT p10/p90: {format::format_ms(metrics.p10_hit_rt_ms)} / {format::format_ms(metrics.p90_hit_rt_ms)}" }
                            }
                        }
                    } else {
                        section { class: "task-nback__metrics task-nback__metrics--placeholder",
                            style: "padding:1rem 1.25rem; border-radius:12px; background:rgba(0,0,0,0.035); color:#5f6575;",
                            p { style: "margin:0;", "Metrics will appear after the first completed session." }
                        }
                    }

                    if let Some(err) = error_message {
                        div { class: "task-nback__error", style: "color:#c21d4a; font-weight:600;", "⚠️ {err}" }
                    }
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

    fn background(&self) -> &'static str {
        match self.tone {
            FeedbackTone::Positive => "rgba(52, 211, 153, 0.9)",
            FeedbackTone::Negative => "rgba(248, 113, 113, 0.92)",
        }
    }

    fn foreground(&self) -> &'static str {
        "#041217"
    }
}

#[derive(Debug, Clone, Copy)]
enum FeedbackTone {
    Positive,
    Negative,
}
