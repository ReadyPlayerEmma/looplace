### looplace-ui.es-ES Fluent messages
### Locale: Spanish (Spain)
### Mirrors the fallback en-US file. Keep keys aligned.
### Add new messages here after adding to en-US to retain compile-time checks.

## Navegación
nav-home = Inicio
nav-pvt = PVT
nav-nback = 2-back
nav-results = Resultados
nav-language-label = Idioma


## Marca y generales
tagline = Rastrea el enfoque con compasión

# $name – nombre para mostrar o identificador corto del usuario.
hello-user = ¡Hola, { $name }!

## Resumen / panel de exportación de resultados
results-title = Resultados de Looplace
results-subtitle-none = Ninguna sesión guardada todavía
results-subtitle-one = 1 sesión guardada localmente
# $count – número entero de sesiones guardadas.
results-subtitle-many = { $count } sesiones guardadas localmente
results-subtitle-all-clean = · todas limpias
# $clean – número entero de sesiones limpias.
results-subtitle-some-clean = · { $clean } limpias
# $when – etiqueta temporal formateada.
results-latest-run = Última sesión { $when }

## Tarjetas destacadas
results-total-runs = Sesiones totales
results-median-pvt = Mediana PVT
results-2back-accuracy = Precisión 2-back
results-2back-dprime = d′ 2-back
# $count – entero
results-clean-sessions-pvt = { $count } sesiones PVT limpias
# $count – entero
results-clean-sessions-nback = { $count } sesiones 2-back limpias
results-signal-detection = Detección de señal a través de sesiones
results-data-pending = Datos pendientes
results-run-pvt-to-populate = Ejecuta un PVT para completar
results-run-nback-to-populate = Completa una sesión 2-back
results-qc-pending = QC pendiente
results-waiting-first = Esperando tu primera sesión

## Tarjeta de la línea PVT
pvt-trend-title = Tendencia PVT
pvt-trend-subtitle = Tiempo de reacción mediano en sesiones limpias
pvt-trend-need-more = Se necesitan más sesiones PVT limpias para trazar la tendencia.

## Tarjeta de barras
bars-title = Lapsos vs falsos inicios
bars-subtitle = Sesiones PVT limpias recientes
bars-need-more = Completa sesiones PVT limpias para comparar lapsos y falsos inicios.

## Leyenda / etiquetas
lapses-label = Lapsos ≥500 ms
false-starts-label = Falsos inicios
# $value – valor numérico redondeado en ms
min-label = MÍN { $value } ms
max-label = MÁX { $value } ms

## Acciones de exportación y estado
export-json = Exportar JSON
export-csv = Exportar CSV
export-png = Exportar PNG
export-working = Generando…
export-done = Listo
export-error = Exportación fallida

## Genéricos
value-missing = —

## Tarea PVT (instrucciones y UI)
pvt-how-summary = Cómo funciona la tarea
pvt-how-step-wait = Espera a que aparezca el contador de milisegundos en el centro.
pvt-how-step-respond = Toca o pulsa espacio en cuanto lo veas—importan la velocidad y la consistencia.
pvt-how-step-jitter = La tarea usa un intervalo variable de 2–10 s; los falsos inicios añaden retraso y los lapsos ≥500 ms se marcan.
# $trials – número entero de reacciones válidas objetivo.
pvt-how-step-target = Cada sesión busca { $trials } reacciones válidas.
pvt-start = Comenzar
pvt-progress-label = Progreso
pvt-last-session = Última sesión
pvt-metrics-placeholder = Las métricas aparecerán tras la primera sesión completa.
pvt-metric-median-rt = RT mediana
pvt-metric-mean-rt = RT media
pvt-metric-sd-rt = RT DE
pvt-metric-p10 = P10
pvt-metric-p90 = P90
pvt-metric-lapses = Lapsos ≥500 ms
pvt-metric-minor-lapses = Lapsos menores 355–499 ms
pvt-metric-false-starts = Falsos inicios
pvt-metric-slope = Pendiente
pvt-metric-min-trials-met = Mín. ensayos cumplidos
pvt-yes = Sí
pvt-no = No
# $message – texto de error
pvt-error-generic = ⚠️ { $message }

## Tarea 2-back (instrucciones y UI)
nback-how-summary = Cómo funciona la tarea
nback-how-step-timing = Cada letra se muestra 0,5 s y luego 2,5 s de intervalo en blanco.
nback-how-step-rule = Pulsa espacio (o toca el panel) cuando la letra coincida con la de dos ensayos atrás.
nback-how-step-practice = El bloque de práctica dura ~35 segundos; la sesión principal unos 3 minutos.
nback-how-step-strategy = Prioriza la precisión antes que la velocidad. Las falsas alarmas afectan a d′ igual que los fallos.
nback-start-practice = Iniciar práctica
nback-start-main = Iniciar sesión principal
nback-practice-recap = Resumen de práctica
# $hits – aciertos; $targets – objetivos; $false_alarms – falsas alarmas; $accuracy – porcentaje redondeado
nback-practice-metrics = Aciertos { $hits } / { $targets } • Falsas alarmas { $false_alarms } • Precisión { $accuracy }%
# $rt – tiempo de reacción formateado
nback-practice-median-hit-rt = RT mediana de aciertos { $rt }
nback-last-session = Última sesión principal
nback-metrics-placeholder = Las métricas aparecerán tras la primera sesión completa.
nback-metric-hits = Aciertos
nback-metric-misses = Fallos
nback-metric-false-alarms = Falsas alarmas
nback-metric-correct-rejections = Rechazos correctos
nback-metric-accuracy = Precisión
nback-metric-dprime = d′
nback-metric-criterion = Criterio
nback-metric-median-hit-rt = RT mediana aciertos
nback-metric-mean-hit-rt = RT media aciertos
nback-metric-hit-rt-p10p90 = RT aciertos p10/p90
# $message – texto de error
nback-error-generic = ⚠️ { $message }

## Comunes (etiquetas compartidas)
common-cancel = Cancelar
common-progress = Progreso

## Guía PVT (textos dinámicos de estado)
pvt-guidance-idle = Pulsa comenzar y espera a que aparezca el contador de milisegundos.
pvt-guidance-waiting = Mantente preparado… el contador aparecerá pronto.
pvt-guidance-active = Toca o pulsa espacio en cuanto aparezca el contador.
pvt-guidance-completed = Sesión completa. Comienza de nuevo cuando estés listo.
pvt-guidance-aborted = Sesión cancelada. Pulsa comenzar para reintentar.

## N-Back etiquetas adicionales
nback-get-ready = Prepárate
nback-mode-practice = Práctica
nback-mode-main = Sesión principal
# Etiqueta accesible para el área de respuesta
nback-aria-respond = Responder a la letra actual

## Página de inicio
home-title = Looplace
home-tagline-short = Bucles pequeños • mentes claras.
home-intro-1 = Rastrea la vigilancia psicomotora y la memoria de trabajo con motores compartidos que se ejecutan en todas partes.
home-feature-pvt = Temporización PVT precisa con métricas locales
home-feature-nback = Sesiones de memoria de trabajo 2-back
home-feature-local = Resultados guardados localmente con opciones de exportación
home-cta = Elige una tarea para comenzar.

## Página PVT
page-pvt-title = Tarea de Vigilancia Psicomotora
page-pvt-intro = Ejecuta un bloque corto de vigilancia para capturar métricas de tiempo de reacción y lapsos.

## Página N-Back
page-nback-title = Memoria de trabajo 2-back
page-nback-intro = Comienza con un breve calentamiento guiado y luego sigue la secuencia 2-back — responde cuando la letra coincida con la de dos ensayos atrás para capturar sensibilidad (d′), sesgo de respuesta y tendencias de tiempo de reacción.

## Página de resultados (extra)
results-page-intro = Revisa resúmenes de sesiones recientes, inspecciona controles de calidad y exporta datos para análisis más profundo.
results-refresh = Actualizar
results-empty = Aún no hay sesiones registradas. Las sesiones completadas aparecerán después de terminar una tarea.
results-error-prefix = ⚠️
