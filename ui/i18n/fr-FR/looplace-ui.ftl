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
nav-language-label = Langue

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

## Tâche PVT (instructions & interface)
pvt-how-summary = Comment fonctionne la tâche
pvt-how-step-wait = Attendez que le compteur de millisecondes apparaisse au centre.
pvt-how-step-respond = Touchez ou appuyez sur espace dès qu’il apparaît — la vitesse et la constance comptent toutes deux.
pvt-how-step-jitter = La tâche utilise un intervalle variable de 2–10 s ; les faux départs ajoutent un délai, les lapses ≥500 ms sont signalés.
# $trials – nombre entier de réactions valides cible.
pvt-how-step-target = Chaque session vise { $trials } réactions valides.
pvt-start = Démarrer
pvt-progress-label = Progression
pvt-last-session = Dernière session
pvt-metrics-placeholder = Les métriques apparaîtront après la première session complète.
pvt-metric-median-rt = RT médiane
pvt-metric-mean-rt = RT moyenne
pvt-metric-sd-rt = RT écart-type
pvt-metric-p10 = P10
pvt-metric-p90 = P90
pvt-metric-lapses = Lapses ≥500 ms
pvt-metric-minor-lapses = Lapses mineurs 355–499 ms
pvt-metric-false-starts = Faux départs
pvt-metric-slope = Pente
pvt-metric-min-trials-met = Min essais atteint
pvt-yes = Oui
pvt-no = Non
# $message – texte d'erreur
pvt-error-generic = ⚠️ { $message }

## Tâche 2-back (instructions & interface)
nback-how-summary = Comment fonctionne la tâche
nback-how-step-timing = Chaque lettre s’affiche 0,5 s puis 2,5 s d’intervalle vide.
nback-how-step-rule = Appuyez sur espace (ou touchez la zone) quand la lettre correspond à celle d’il y a deux essais.
nback-how-step-practice = Le bloc d’entraînement dure ~35 secondes ; la session principale environ 3 minutes.
nback-how-step-strategy = Priorisez la précision avant la vitesse. Les fausses alertes pénalisent d′ autant que les omissions.
nback-start-practice = Démarrer entraînement
nback-start-main = Démarrer session principale
nback-practice-recap = Récap entraînement
# $hits – succès; $targets – cibles; $false_alarms – fausses alertes; $accuracy – pourcentage arrondi
nback-practice-metrics = Succès { $hits } / { $targets } • Fausses alertes { $false_alarms } • Précision { $accuracy }%
# $rt – temps de réaction formatté
nback-practice-median-hit-rt = RT médiane succès { $rt }
nback-last-session = Dernière session principale
nback-metrics-placeholder = Les métriques apparaîtront après la première session complète.
nback-metric-hits = Succès
nback-metric-misses = Omissions
nback-metric-false-alarms = Fausses alertes
nback-metric-correct-rejections = Rejets corrects
nback-metric-accuracy = Précision
nback-metric-dprime = d′
nback-metric-criterion = Critère
nback-metric-median-hit-rt = RT médiane succès
nback-metric-mean-hit-rt = RT moyenne succès
nback-metric-hit-rt-p10p90 = RT succès p10/p90
# $message – texte d'erreur
nback-error-generic = ⚠️ { $message }

## Commun (étiquettes partagées)
common-cancel = Annuler
common-progress = Progression

## Guidance PVT (textes d’état dynamiques)
pvt-guidance-idle = Appuyez sur démarrer puis attendez que le compteur de millisecondes apparaisse.
pvt-guidance-waiting = Restez prêt… le compteur apparaîtra bientôt.
pvt-guidance-active = Touchez ou appuyez sur espace dès que le compteur apparaît.
pvt-guidance-completed = Session terminée. Redémarrez quand vous voulez.
pvt-guidance-aborted = Session annulée. Relancez pour réessayer.

## N-Back libellés supplémentaires
nback-get-ready = Préparez-vous
nback-mode-practice = Entraînement
nback-mode-main = Session principale
# Libellé accessibilité pour la zone de réponse
nback-aria-respond = Répondre à la lettre actuelle

## Page d’accueil
home-title = Looplace
home-tagline-short = Petites boucles • esprit clair.
home-intro-1 = Suivez la vigilance psychomotrice et la mémoire de travail avec des moteurs partagés qui s’exécutent partout.
home-feature-pvt = Chronométrage PVT précis avec métriques locales
home-feature-nback = Sessions de mémoire de travail 2-back
home-feature-local = Résultats stockés localement avec options d’export
home-cta = Choisissez une tâche pour commencer.

## Page PVT
page-pvt-title = Tâche de Vigilance Psychomotrice
page-pvt-intro = Lancez un court bloc de vigilance pour capturer temps de réaction et lapses.

## Page N-Back
page-nback-title = Mémoire de travail 2-back
page-nback-intro = Commencez par un court échauffement guidé puis suivez le flux 2-back — répondez quand la lettre correspond à celle d’il y a deux essais pour capturer sensibilité (d′), biais de réponse et tendances de temps de réaction.

## Page Résultats (supplément)
results-page-intro = Consultez les résumés des sessions récentes, inspectez les contrôles de qualité et exportez les données pour une analyse approfondie.
results-refresh = Actualiser
results-empty = Aucune session enregistrée. Les sessions complétées apparaîtront après avoir terminé une tâche.
results-error-prefix = ⚠️
