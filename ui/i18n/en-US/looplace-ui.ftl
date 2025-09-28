### looplace-ui.en-US Fluent messages
### Fallback / reference locale.
### Guidelines:
### - Keep IDs kebab-case.
### - Document variable usage with comments directly above each message.
### - When adding a new message, add it here first so compile-time checks pass.

## Navigation
nav-home = Home
nav-pvt = PVT
nav-nback = 2-back
nav-results = Results
nav-language-label = Language

## Brand & general
tagline = Track focus with compassion

# $name – display name or short identifier of the user.
hello-user = Hello, { $name }!

## Results snapshot / export panel
results-title = Looplace results
results-subtitle-none = No runs saved yet
results-subtitle-one = 1 run saved locally
# $count – integer number of saved runs.
results-subtitle-many = { $count } runs saved locally
results-subtitle-all-clean = · all clean
# $clean – integer number of clean runs.
results-subtitle-some-clean = · { $clean } clean
results-latest-run = Latest run { $when }

## Highlight cards
results-total-runs = Total runs
results-median-pvt = Median PVT
results-2back-accuracy = 2-back accuracy
results-2back-dprime = 2-back d′
results-clean-sessions-pvt = { $count } clean PVT sessions
results-clean-sessions-nback = { $count } clean 2-back sessions
results-signal-detection = Signal detection across sessions
results-data-pending = Data pending
results-run-pvt-to-populate = Run a PVT to populate
results-run-nback-to-populate = Complete a 2-back session
results-qc-pending = QC pending
results-waiting-first = Waiting on your first session

## PVT sparkline card
pvt-trend-title = PVT trend
pvt-trend-subtitle = Median reaction time across clean runs
pvt-trend-need-more = Need more clean PVT runs to plot a trend.

## Bars card
bars-title = Lapses vs false starts
bars-subtitle = Recent clean PVT sessions
bars-need-more = Complete clean PVT runs to compare lapses and false starts.

## Legend / labels
lapses-label = Lapses ≥500 ms
false-starts-label = False starts
min-label = MIN { $value } ms
max-label = MAX { $value } ms

## Export actions & status
export-json = Export JSON
export-csv = Export CSV
export-png = Export PNG
export-working = Building…
export-done = Done
export-error = Export failed

## Generic fallbacks / symbols
value-missing = —

## PVT task (instructions & UI)
pvt-how-summary = How the task works
pvt-how-step-wait = Wait for the milliseconds counter to appear in the centre.
pvt-how-step-respond = Tap or press space as soon as you see it—speed and consistency both matter.
pvt-how-step-jitter = Runs use 2–10 s jitter; false starts add delay, lapses ≥500 ms are flagged.
# $trials – integer number of target valid reactions in a session.
pvt-how-step-target = Each session targets { $trials } valid reactions.
pvt-start = Start
pvt-progress-label = Progress
pvt-last-session = Last session
pvt-metrics-placeholder = Metrics will appear after the first completed session.
pvt-metric-median-rt = Median RT
pvt-metric-mean-rt = Mean RT
pvt-metric-sd-rt = SD RT
pvt-metric-p10 = P10
pvt-metric-p90 = P90
pvt-metric-lapses = Lapses ≥500 ms
pvt-metric-minor-lapses = Minor lapses 355–499 ms
pvt-metric-false-starts = False starts
pvt-metric-slope = Slope
pvt-metric-min-trials-met = Min trials met
pvt-yes = Yes
pvt-no = No
pvt-error-generic = ⚠️ { $message }

## N-Back (2-back) task (instructions & UI)
nback-how-summary = How the task works
nback-how-step-timing = Each letter displays for 0.5 s, followed by 2.5 s of blank interval.
nback-how-step-rule = Press space (or tap the pad) whenever the letter matches the one from two trials ago.
nback-how-step-practice = Practice block lasts ~35 seconds; main run is about 3 minutes.
nback-how-step-strategy = Focus on accuracy first, then speed. False alarms tax d′ just like misses.
nback-start-practice = Start practice
nback-start-main = Start main session
nback-practice-recap = Practice recap
# $hits – number of hits; $targets – number of target trials; $false_alarms – number of false alarms; $accuracy – rounded percentage (no % sign inside value)
nback-practice-metrics = Hits { $hits } / { $targets } • False alarms { $false_alarms } • Accuracy { $accuracy }%
# $rt – formatted reaction time (e.g. "350 ms")
nback-practice-median-hit-rt = Median hit RT { $rt }
nback-last-session = Last main session
nback-metrics-placeholder = Metrics will appear after the first completed session.
nback-metric-hits = Hits
nback-metric-misses = Misses
nback-metric-false-alarms = False alarms
nback-metric-correct-rejections = Correct rejections
nback-metric-accuracy = Accuracy
nback-metric-dprime = d′
nback-metric-criterion = Criterion
nback-metric-median-hit-rt = Median hit RT
nback-metric-mean-hit-rt = Mean hit RT
nback-metric-hit-rt-p10p90 = Hit RT p10/p90
nback-error-generic = ⚠️ { $message }

## Common (shared UI labels)
common-cancel = Cancel
common-progress = Progress

## PVT guidance (dynamic status text)
pvt-guidance-idle = Press start, then wait for the milliseconds counter to appear.
pvt-guidance-waiting = Hold steady… the counter will appear soon.
pvt-guidance-active = Tap or press space the moment the counter appears.
pvt-guidance-completed = Session complete. Start again when ready.
pvt-guidance-aborted = Run cancelled. Start to retry.

## N-Back additional labels
nback-get-ready = Get ready
nback-mode-practice = Practice
nback-mode-main = Main session
# Accessibility label for reaction target / hitbox
nback-aria-respond = Respond to current letter

## Home page
home-title = Looplace
home-tagline-short = Small loops • clear minds.
home-intro-1 = Track psychomotor vigilance and working memory with shared engines that run everywhere.
home-feature-pvt = Precise PVT timing with local metrics
home-feature-nback = 2-back working memory sessions
home-feature-local = Results stored locally with export options
home-cta = Choose a task to get started.

## PVT page
page-pvt-title = Psychomotor Vigilance Task
page-pvt-intro = Run a short vigilance block to capture reaction time metrics and lapse counts.

## N-Back page
page-nback-title = 2-back Working Memory
page-nback-intro = Start with a short guided warm-up, then follow the 2-back stream—respond whenever the letter matches the one from two trials ago to capture sensitivity (d′), response bias, and reaction-time trends.

## Results page (extra)
results-page-intro = Review summaries from recent runs, inspect quality checks, and export data for deeper analysis.
results-refresh = Refresh
results-empty = No runs recorded yet. Completed sessions will appear after you finish a task.
results-error-prefix = ⚠️
