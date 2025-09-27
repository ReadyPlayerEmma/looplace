### looplace-ui.fr-FR Fluent messages
### Locale: French (France)
### NOTE:
### - This is an initial draft translation. Some domain-specific terms (e.g. “PVT”, “2-back”, “lapses”)
###   are intentionally kept close to English for clarity and can be refined with native feedback.
### - Keep keys identical to the fallback (en-US) file.
### - Variables (e.g. { $name }, { $count }) MUST remain unchanged.

## Navigation
nav-home = Accueil
nav-pvt = PVT
nav-nback = 2-back
nav-results = Résultats

## Brand & general
tagline = Suivre l’attention avec compassion

# $name – display name or short identifier of the user.
hello-user = Bonjour, { $name } !

## Results snapshot / export panel
results-title = Résultats Looplace
results-subtitle-none = Aucune session enregistrée
results-subtitle-one = 1 session enregistrée localement
# $count – integer number of saved runs.
results-subtitle-many = { $count } sessions enregistrées localement
results-subtitle-all-clean = · toutes propres
# $clean – integer number of clean runs.
results-subtitle-some-clean = · { $clean } propres
results-latest-run = Dernière session { $when }

## Highlight cards
results-total-runs = Sessions totales
results-median-pvt = Médiane PVT
results-2back-accuracy = Précision 2-back
results-2back-dprime = d′ 2-back
results-clean-sessions-pvt = { $count } sessions PVT propres
results-clean-sessions-nback = { $count } sessions 2-back propres
results-signal-detection = Détection du signal sur l’ensemble des sessions
results-data-pending = Données en attente
results-run-pvt-to-populate = Effectuez un PVT pour remplir
results-run-nback-to-populate = Effectuez une session 2-back
results-qc-pending = QC en attente
results-waiting-first = En attente de votre première session

## PVT sparkline card
pvt-trend-title = Tendance PVT
pvt-trend-subtitle = Temps de réaction médian (sessions propres)
pvt-trend-need-more = Plus de sessions PVT propres nécessaires pour tracer une tendance.

## Bars card
bars-title = Lapses vs faux départs
bars-subtitle = Sessions PVT propres récentes
bars-need-more = Effectuez des sessions PVT propres pour comparer lapses et faux départs.

## Legend / labels
lapses-label = Lapses ≥500 ms
false-starts-label = Faux départs
# $value – rounded numeric value in ms
min-label = MIN { $value } ms
max-label = MAX { $value } ms

## Export actions & status
export-json = Exporter JSON
export-csv = Exporter CSV
export-png = Exporter PNG
export-working = Génération…
export-done = Terminé
export-error = Échec de l’export

## Generic fallbacks / symbols
value-missing = —
