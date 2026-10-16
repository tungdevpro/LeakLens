//! A hand-drawn SVG line chart over Session time.

use std::rc::Rc;
use std::time::Duration;

use dioxus::prelude::*;

const LEFT: f64 = 52.0;
const RIGHT: f64 = 12.0;
const TOP: f64 = 10.0;
const BOTTOM: f64 = 22.0;

/// The x axis always spans at least this much, so a new Session doesn't
/// stretch its first few points across the whole width.
const MIN_SPAN: Duration = Duration::from_secs(60);

#[derive(Debug, Clone, PartialEq)]
pub struct Point {
    pub at: Duration,
    pub value: f64,
}

/// One line on a chart. Series take the categorical colours in order.
#[derive(Debug, Clone, PartialEq)]
pub struct Series {
    pub name: String,
    pub points: Vec<Point>,
}

/// A line chart sharing one y axis across its series.
///
/// `width` and `height` set the drawing's own units; it scales to fit its box,
/// so a smaller `width` keeps labels readable in a narrow box.
///
/// - `y_floor`: the smallest top of the y axis; it grows to fit larger values.
/// - `max_gap`: points further apart than this are not joined, so missing
///   Samples show as gaps rather than invented lines.
#[component]
pub fn LineChart(
    title: String,
    unit: String,
    #[props(default = 0)] decimals: usize,
    y_floor: f64,
    max_gap: Duration,
    series: Vec<Series>,
    #[props(default = 800.0)] width: f64,
    #[props(default = 180.0)] height: f64,
) -> Element {
    let mut hover = use_signal(|| None::<Duration>);
    let mut svg = use_signal(|| None::<Rc<MountedData>>);

    let last_at = series
        .iter()
        .filter_map(|s| s.points.last())
        .map(|p| p.at)
        .max();
    let span = last_at.map_or(MIN_SPAN, |at| at.max(MIN_SPAN));
    let peak = series
        .iter()
        .flat_map(|s| &s.points)
        .map(|p| p.value)
        .fold(0.0, f64::max);
    // Four gridline steps of a round size (1, 2 or 5 times a power of ten),
    // with the top of the axis on the first step at or above the data.
    let y_step = nice_ceiling(peak.max(y_floor) / 4.0);
    let y_max = y_step * (peak.max(y_floor) / y_step).ceil().max(1.0);

    let plot_w = width - LEFT - RIGHT;
    let plot_h = height - TOP - BOTTOM;
    let x = move |at: Duration| LEFT + at.as_secs_f64() / span.as_secs_f64() * plot_w;
    let y = move |value: f64| TOP + plot_h - value / y_max * plot_h;

    let y_ticks: Vec<f64> = (0..)
        .map(|i| y_step * f64::from(i))
        .take_while(|v| *v <= y_max + y_step / 2.0)
        .collect();
    let y_decimals = if y_step < 1.0 { 1 } else { 0 };
    let x_step = time_step(span);
    let x_ticks: Vec<Duration> = (0..)
        .map(|i| x_step * i)
        .take_while(|t| *t <= span)
        .collect();
    let is_empty = series.iter().all(|s| s.points.is_empty());

    // Values shown in the legend: at the hovered moment, otherwise the latest.
    let hovered = hover();
    let shown: Vec<Option<f64>> = series
        .iter()
        .map(|s| match hovered {
            Some(at) => nearest(&s.points, at, max_gap).map(|p| p.value),
            None => s.points.last().map(|p| p.value),
        })
        .collect();
    let unit_label = unit.clone();
    let format = move |value: Option<f64>| match value {
        Some(v) => format!("{v:.decimals$}{unit}"),
        None => "—".to_owned(),
    };
    let single = series.len() == 1;

    rsx! {
        figure { class: "chart",
            figcaption {
                span { class: "chart-title", "{title}" }
                if single {
                    span { class: "chart-value", {format(shown[0])} }
                }
            }
            if !single {
                ul { class: "legend",
                    for (index, (s, value)) in series.iter().zip(shown.iter().copied()).enumerate() {
                        li {
                            span { class: "swatch series-{index + 1}" }
                            span { class: "legend-name", "{s.name}" }
                            span { class: "chart-value", {format(value)} }
                        }
                    }
                }
            }
            svg {
                view_box: "0 0 {width} {height}",
                role: "img",
                "aria-label": "{title} over Session time",
                onmounted: move |e| svg.set(Some(e.data())),
                onmouseleave: move |_| hover.set(None),
                onmousemove: move |e| {
                    let offset = e.element_coordinates().x;
                    async move {
                        let Some(mounted) = svg() else { return };
                        let Ok(rect) = mounted.get_client_rect().await else { return };
                        let scale = width / rect.width();
                        let plot_x = (offset * scale - LEFT).clamp(0.0, plot_w);
                        hover.set(Some(span.mul_f64(plot_x / plot_w)));
                    }
                },
                for value in y_ticks {
                    line {
                        class: "grid",
                        x1: "{LEFT}", x2: "{width - RIGHT}",
                        y1: "{y(value)}", y2: "{y(value)}",
                    }
                    text {
                        class: "axis-label",
                        x: "{LEFT - 6.0}", y: "{y(value) + 4.0}",
                        text_anchor: "end",
                        "{value:.y_decimals$}{unit_label}"
                    }
                }
                for at in x_ticks {
                    text {
                        class: "axis-label",
                        x: "{x(at)}", y: "{height - 6.0}",
                        text_anchor: "middle",
                        {format_time(at)}
                    }
                }
                if is_empty {
                    text {
                        class: "chart-empty",
                        x: "{LEFT + plot_w / 2.0}", y: "{TOP + plot_h / 2.0}",
                        text_anchor: "middle",
                        "Waiting for the first Sample…"
                    }
                }
                for (index, s) in series.iter().enumerate() {
                    for run in runs(&s.points, max_gap) {
                        if let [only] = run {
                            circle { class: "dot series-{index + 1}", cx: "{x(only.at)}", cy: "{y(only.value)}", r: "2" }
                        } else {
                            polyline {
                                class: "line series-{index + 1}",
                                points: run.iter().map(|p| format!("{:.1},{:.1}", x(p.at), y(p.value))).collect::<Vec<_>>().join(" "),
                            }
                        }
                    }
                }
                if let Some(at) = hovered {
                    line {
                        class: "crosshair",
                        x1: "{x(at)}", x2: "{x(at)}",
                        y1: "{TOP}", y2: "{TOP + plot_h}",
                    }
                    for (index, s) in series.iter().enumerate() {
                        if let Some(p) = nearest(&s.points, at, max_gap) {
                            circle { class: "marker series-{index + 1}", cx: "{x(p.at)}", cy: "{y(p.value)}", r: "4" }
                        }
                    }
                    text {
                        class: "axis-label",
                        x: "{x(at)}", y: "{TOP + 10.0}",
                        text_anchor: if x(at) > width / 2.0 { "end" } else { "start" },
                        dx: if x(at) > width / 2.0 { "-6" } else { "6" },
                        {format_time(at)}
                    }
                }
            }
        }
    }
}

/// Splits points into runs that are joined by a line, breaking where two
/// neighbours are further apart than `max_gap`.
fn runs(points: &[Point], max_gap: Duration) -> Vec<&[Point]> {
    let mut runs = Vec::new();
    let mut start = 0;
    for i in 1..points.len() {
        if points[i].at.saturating_sub(points[i - 1].at) > max_gap {
            runs.push(&points[start..i]);
            start = i;
        }
    }
    if start < points.len() {
        runs.push(&points[start..]);
    }
    runs
}

/// The point closest to `at`, if one lies within `max_gap` of it.
fn nearest(points: &[Point], at: Duration, max_gap: Duration) -> Option<&Point> {
    points
        .iter()
        .min_by_key(|p| p.at.abs_diff(at))
        .filter(|p| p.at.abs_diff(at) <= max_gap)
}

/// Rounds up to 1, 2 or 5 times a power of ten, so axis labels stay readable.
fn nice_ceiling(value: f64) -> f64 {
    if value <= 0.0 {
        return 1.0;
    }
    let magnitude = 10f64.powf(value.log10().floor());
    [1.0, 2.0, 5.0, 10.0]
        .into_iter()
        .map(|m| m * magnitude)
        .find(|v| *v >= value)
        .unwrap()
}

/// A tick spacing that gives roughly six labels across `span`.
fn time_step(span: Duration) -> Duration {
    [10, 15, 30, 60, 120, 300, 600, 900, 1800, 3600]
        .into_iter()
        .map(Duration::from_secs)
        .find(|step| span.as_secs_f64() / step.as_secs_f64() <= 6.0)
        .unwrap_or(Duration::from_secs(3600))
}

fn format_time(at: Duration) -> String {
    let secs = at.as_secs();
    format!("{}:{:02}", secs / 60, secs % 60)
}
