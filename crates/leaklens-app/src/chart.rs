//! A hand-drawn SVG line chart over Session time.

use std::time::Duration;

use dioxus::prelude::*;

const WIDTH: f64 = 800.0;
const HEIGHT: f64 = 180.0;
const LEFT: f64 = 48.0;
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

/// `y_floor` is the smallest top of the y axis; the axis grows to fit larger values.
#[component]
pub fn LineChart(title: String, unit: String, y_floor: f64, points: Vec<Point>) -> Element {
    let span = points.last().map_or(MIN_SPAN, |p| p.at.max(MIN_SPAN));
    let peak = points.iter().map(|p| p.value).fold(0.0, f64::max);
    let y_max = nice_ceiling(peak.max(y_floor));

    let plot_w = WIDTH - LEFT - RIGHT;
    let plot_h = HEIGHT - TOP - BOTTOM;
    let x = |at: Duration| LEFT + at.as_secs_f64() / span.as_secs_f64() * plot_w;
    let y = |value: f64| TOP + plot_h - value / y_max * plot_h;

    let line: String = points
        .iter()
        .map(|p| format!("{:.1},{:.1}", x(p.at), y(p.value)))
        .collect::<Vec<_>>()
        .join(" ");
    let y_ticks: Vec<f64> = (0..=4).map(|i| y_max * f64::from(i) / 4.0).collect();
    let x_step = time_step(span);
    let x_ticks: Vec<Duration> = (0..)
        .map(|i| x_step * i)
        .take_while(|t| *t <= span)
        .collect();
    let latest = points
        .last()
        .map(|p| format!("{:.1}{unit}", p.value))
        .unwrap_or_else(|| "—".to_owned());

    rsx! {
        figure { class: "chart",
            figcaption {
                span { class: "chart-title", "{title}" }
                span { class: "chart-latest", "{latest}" }
            }
            svg {
                view_box: "0 0 {WIDTH} {HEIGHT}",
                role: "img",
                "aria-label": "{title} over time",
                for value in y_ticks {
                    line {
                        class: "grid",
                        x1: "{LEFT}", x2: "{WIDTH - RIGHT}",
                        y1: "{y(value)}", y2: "{y(value)}",
                    }
                    text {
                        class: "axis-label",
                        x: "{LEFT - 6.0}", y: "{y(value) + 4.0}",
                        text_anchor: "end",
                        "{value:.0}{unit}"
                    }
                }
                for at in x_ticks {
                    text {
                        class: "axis-label",
                        x: "{x(at)}", y: "{HEIGHT - 6.0}",
                        text_anchor: "middle",
                        {format_time(at)}
                    }
                }
                if points.is_empty() {
                    text {
                        class: "chart-empty",
                        x: "{LEFT + plot_w / 2.0}", y: "{TOP + plot_h / 2.0}",
                        text_anchor: "middle",
                        "Waiting for the first Sample…"
                    }
                } else {
                    polyline { class: "series", points: "{line}" }
                }
            }
        }
    }
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
