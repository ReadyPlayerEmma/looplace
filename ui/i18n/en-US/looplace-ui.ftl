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
