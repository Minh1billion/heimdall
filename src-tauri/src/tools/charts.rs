use crate::models::{clip, py_round, utcnow};
use crate::ports::tool::{Tool, ToolContext, ToolError, ToolFailure};
use crate::services::ads::currency_for;
use crate::services::query::{aggregate, resolve_range};
use crate::tools::common::{check_account, int_arg, list_arg, text_arg, today, METRICS, RANGE_HELP};
use crate::validation::{Field, Kind};
use async_trait::async_trait;
use rusqlite::params;
use serde_json::{json, Map, Value};

const MAX_CHARTS: i64 = 3;
const MAX_POINTS: usize = 60;
const MAX_SCALE_GAP: f64 = 20.0;

fn meta(metric: &str) -> (&'static str, &'static str) {
    match metric {
        "spend" => ("Spend", "money"),
        "impressions" => ("Impressions", "count"),
        "clicks" => ("Clicks", "count"),
        "conversions" => ("Conversions", "count"),
        "ctr" => ("CTR", "percent"),
        "cpc" => ("CPC", "money"),
        "cpm" => ("CPM", "money"),
        "cpa" => ("CPA", "money"),
        "roas" => ("ROAS", "ratio"),
        _ => ("Metric", "count"),
    }
}

fn failure(code: &str, message: &str, hint: &str) -> ToolError {
    ToolError::Failure(ToolFailure::with_hint(code, message, hint))
}

fn row_label(row: &Map<String, Value>, level: &str) -> String {
    let key = match level {
        "day" => "day",
        "account" => "account_name",
        _ => "campaign_name",
    };
    let text = row.get(key).and_then(Value::as_str).unwrap_or("");
    clip(text, 40)
}

fn metric_of(row: &Map<String, Value>, metric: &str) -> Option<f64> {
    row.get(metric).and_then(Value::as_f64).filter(|value| value.is_finite())
}

fn summarize(data: &[Value], metric: &str) -> Value {
    let points: Vec<(usize, f64)> = data
        .iter()
        .enumerate()
        .filter_map(|(index, item)| item.get(metric).and_then(Value::as_f64).map(|value| (index, value)))
        .collect();
    if points.is_empty() {
        return Value::Null;
    }
    let label_at = |index: usize| -> Value { data[index].get("label").cloned().unwrap_or(Value::Null) };
    let mut low = points[0];
    let mut high = points[0];
    for point in &points {
        if point.1 < low.1 {
            low = *point;
        }
        if point.1 > high.1 {
            high = *point;
        }
    }
    let first = points[0];
    let last = points[points.len() - 1];
    json!({
        "first": first.1,
        "first_at": label_at(first.0),
        "last": last.1,
        "last_at": label_at(last.0),
        "min": low.1,
        "min_at": label_at(low.0),
        "max": high.1,
        "max_at": label_at(high.0),
    })
}

pub struct RenderChart;

#[async_trait]
impl Tool for RenderChart {
    fn name(&self) -> &'static str {
        "render_chart"
    }

    fn description(&self) -> &'static str {
        "Draw a line or bar chart from the synced ad data and show it to the user in the chat. Call it yourself whenever a chart answers the question better than text: the user asks to plot, chart, graph or visualize, wants a trend over several days, or wants several campaigns or accounts compared on a metric. Do not call it for a single number, a two-value comparison, or when the user wants text only. Use level=day with chart_type=line for trends over time, and level=campaign or account with chart_type=bar for comparisons. Prefer one metric per chart. Several metrics are allowed only when they share a unit or there are at most two different units. The data comes straight from the database, so never pass numbers. Returns summary statistics for your short commentary."
    }

    fn fields(&self) -> Vec<Field> {
        vec![
            Field::text("account_id")
                .nullish()
                .describe("Ad account id such as act_123. Omit for all accounts."),
            Field::new("level", Kind::Enum(&["day", "campaign", "account"])).with_default(json!("day")),
            Field::text("date_range").with_default(json!("last_7d")).describe(RANGE_HELP),
            Field::new("metrics", Kind::EnumList(&METRICS))
                .with_default(json!(["spend"]))
                .describe("1 to 3 metrics to plot."),
            Field::new("chart_type", Kind::Enum(&["line", "bar"])).with_default(json!("line")),
            Field::new("limit", Kind::Int { min: Some(1), max: Some(15) })
                .with_default(json!(10))
                .describe("Maximum campaigns or accounts shown. Ignored for level=day."),
            Field::new("sort_by", Kind::Enum(&METRICS))
                .nullish()
                .describe("Metric used to pick the top rows for campaign or account level. Defaults to the first metric."),
            Field::new("title", Kind::Str { min: None, max: Some(80) }).nullish(),
        ]
    }

    async fn run(&self, ctx: &ToolContext, args: &Map<String, Value>) -> Result<Value, ToolError> {
        {
            let conn = ctx.container.db.lock();
            let existing: i64 = conn.query_row(
                "SELECT COUNT(*) FROM run_charts WHERE run_id = ?1",
                [&ctx.run_id],
                |row| row.get(0),
            )?;
            if existing >= MAX_CHARTS {
                return Err(failure(
                    "chart_limit",
                    "Too many charts in one answer",
                    "Answer with the charts already shown",
                ));
            }
        }
        let (start, end) = resolve_range(text_arg(args, "date_range").unwrap_or("last_7d"), &today())?;
        let account = text_arg(args, "account_id");
        check_account(ctx, account)?;
        let level = text_arg(args, "level").unwrap_or("day").to_string();
        let chart_type = text_arg(args, "chart_type").unwrap_or("line").to_string();

        let mut metrics: Vec<String> = Vec::new();
        for metric in list_arg(args, "metrics") {
            if !metrics.contains(&metric) {
                metrics.push(metric);
            }
        }
        if metrics.is_empty() {
            metrics.push("spend".to_string());
        }
        if metrics.len() > 3 {
            return Err(failure("too_many_metrics", "At most 3 metrics per chart", "Split them into separate charts"));
        }
        let mut units: Vec<&'static str> = Vec::new();
        for metric in &metrics {
            let unit = meta(metric).1;
            if !units.contains(&unit) {
                units.push(unit);
            }
        }
        if units.len() > 2 {
            return Err(failure(
                "too_many_units",
                "A chart supports at most two different units",
                "Split the metrics into separate charts",
            ));
        }

        let mut rows = aggregate(&ctx.container.db, &start, &end, account, &level)?;
        if rows.is_empty() {
            return Err(failure(
                "no_data",
                "No data in this period",
                "The account may not be synced yet",
            ));
        }
        if level == "day" {
            if rows.len() > MAX_POINTS {
                let cut = rows.len() - MAX_POINTS;
                rows.drain(0..cut);
            }
        } else {
            let sort_key = text_arg(args, "sort_by").unwrap_or(metrics[0].as_str()).to_string();
            rows.sort_by(|left, right| {
                let a = metric_of(left, &sort_key).unwrap_or(f64::NEG_INFINITY);
                let b = metric_of(right, &sort_key).unwrap_or(f64::NEG_INFINITY);
                b.partial_cmp(&a).unwrap_or(std::cmp::Ordering::Equal)
            });
            rows.truncate(int_arg(args, "limit").max(1) as usize);
        }

        for unit in &units {
            let peaks: Vec<f64> = metrics
                .iter()
                .filter(|metric| meta(metric).1 == *unit)
                .map(|metric| {
                    rows.iter()
                        .filter_map(|row| metric_of(row, metric))
                        .fold(0.0_f64, |acc, value| acc.max(value.abs()))
                })
                .filter(|peak| *peak > 0.0)
                .collect();
            if peaks.len() > 1 {
                let high = peaks.iter().cloned().fold(f64::MIN, f64::max);
                let low = peaks.iter().cloned().fold(f64::MAX, f64::min);
                if high / low > MAX_SCALE_GAP {
                    return Err(failure(
                        "scale_mismatch",
                        "These metrics differ too much in scale to share one chart",
                        "Render them as separate charts",
                    ));
                }
            }
        }

        let data: Vec<Value> = rows
            .iter()
            .map(|row| {
                let mut item: Map<String, Value> = Map::new();
                item.insert("label".to_string(), json!(row_label(row, &level)));
                for metric in &metrics {
                    let value = match metric_of(row, metric) {
                        Some(found) => json!(py_round(found, 4)),
                        None => Value::Null,
                    };
                    item.insert(metric.clone(), value);
                }
                Value::Object(item)
            })
            .collect();

        let series: Vec<Value> = metrics
            .iter()
            .map(|metric| {
                let (name, unit) = meta(metric);
                json!({ "key": metric, "name": name, "unit": unit })
            })
            .collect();
        let currency = currency_for(&ctx.container.db, &start, &end, account)?;
        let title = text_arg(args, "title")
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(String::from)
            .unwrap_or_else(|| {
                let names: Vec<&str> = metrics.iter().map(|metric| meta(metric).0).collect();
                let by = match level.as_str() {
                    "day" => "by day",
                    "account" => "by account",
                    _ => "by campaign",
                };
                format!("{} {}", names.join(" and "), by)
            });

        let spec = json!({
            "type": chart_type,
            "title": title,
            "level": level,
            "start": start,
            "end": end,
            "currency": currency,
            "series": series,
            "data": data,
        });
        {
            let conn = ctx.container.db.lock();
            conn.execute(
                "INSERT INTO run_charts (run_id, spec, created_at) VALUES (?1, ?2, ?3)",
                params![ctx.run_id, spec.to_string(), utcnow()],
            )?;
        }

        let mut summary: Map<String, Value> = Map::new();
        for metric in &metrics {
            summary.insert(metric.clone(), summarize(&data, metric));
        }
        Ok(json!({
            "ok": true,
            "chart_shown": true,
            "title": title,
            "level": level,
            "start": start,
            "end": end,
            "currency": currency,
            "points": data.len(),
            "summary": summary,
            "note": "The chart is already displayed to the user. Do not repeat the values. Give one to three short sentences with the key takeaways, using only these summary figures or other tool results.",
        }))
    }
}
