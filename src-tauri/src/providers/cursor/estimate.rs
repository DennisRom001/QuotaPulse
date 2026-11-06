//! Estimated Cursor plan usage from local prompt counts.
//!
//! Cursor doesn't store its quota on disk, so this is an approximation:
//!
//!   used%  ≈  prompts sent this billing cycle / monthly prompt budget
//!
//! The budget is the number of prompts that would use up the plan. The
//! user can type it, or — the accurate way — *calibrate* it: read the real
//! percentage on cursor.com/dashboard once, and QuotaPulse sets
//! `budget = prompts so far / real%`. Right after calibrating, the estimate
//! matches the dashboard; it then stays close as long as the user's mix of
//! models and prompt sizes stays similar (Cursor bills by model cost, which
//! is not visible locally). Re-calibrating occasionally keeps it close.
//!
//! Lines suggested/accepted are shown on the card but are not part of the
//! formula: Cursor's quota counts model requests, not lines.

use std::sync::RwLock;

use chrono::{DateTime, Datelike, Duration, Local, Months, NaiveDate, TimeZone, Utc};

use super::local_activity::CursorActivity;
use crate::usage::models::{UsageUnit, UsageWindow, WindowType};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EstimateConfig {
    /// Prompts per billing cycle that use up the plan. `None` = no estimate.
    pub monthly_prompts: Option<u32>,
    /// Day of the month the billing cycle starts (1–28).
    pub cycle_start_day: u8,
}

impl Default for EstimateConfig {
    fn default() -> Self {
        Self {
            monthly_prompts: None,
            cycle_start_day: 1,
        }
    }
}

static CONFIG: RwLock<EstimateConfig> = RwLock::new(EstimateConfig {
    monthly_prompts: None,
    cycle_start_day: 1,
});

pub fn set_config(config: EstimateConfig) {
    if let Ok(mut c) = CONFIG.write() {
        *c = EstimateConfig {
            monthly_prompts: config.monthly_prompts.filter(|n| *n > 0),
            cycle_start_day: config.cycle_start_day.clamp(1, 28),
        };
    }
}

pub fn config() -> EstimateConfig {
    CONFIG.read().map(|c| *c).unwrap_or_default()
}

/// Start and end of the billing cycle containing `today`.
pub fn cycle_bounds(today: NaiveDate, start_day: u8) -> (DateTime<Utc>, DateTime<Utc>) {
    let day = u32::from(start_day.clamp(1, 28));
    let this_month = today.with_day(day).expect("day 1-28 exists in every month");
    let start = if today >= this_month {
        this_month
    } else {
        this_month - Months::new(1)
    };
    let end = start + Months::new(1);
    let to_utc = |d: NaiveDate| {
        Local
            .from_local_datetime(&d.and_hms_opt(0, 0, 0).expect("midnight"))
            .earliest()
            .map(|t| t.with_timezone(&Utc))
            .unwrap_or_else(|| d.and_hms_opt(0, 0, 0).expect("midnight").and_utc())
    };
    (to_utc(start), to_utc(end))
}

#[derive(Debug, Clone, PartialEq)]
pub struct Estimate {
    pub used: u32,
    pub budget: u32,
    pub percent: f64,
    /// Percent the cycle would end at if the current pace holds.
    pub projected_percent: f64,
    pub resets_at: DateTime<Utc>,
}

pub fn estimate(
    activity: &CursorActivity,
    config: EstimateConfig,
    now: DateTime<Utc>,
) -> Option<Estimate> {
    let budget = config.monthly_prompts.filter(|n| *n > 0)?;
    let (start, end) = cycle_bounds(now.with_timezone(&Local).date_naive(), config.cycle_start_day);
    let used = activity.prompts_since(start);
    let percent = f64::from(used) / f64::from(budget) * 100.0;
    // Pace: prompts per elapsed hour, extended to the whole cycle. At least
    // one day elapsed, so the first hours of a cycle don't explode the figure.
    let elapsed = (now - start).max(Duration::days(1)).num_seconds() as f64;
    let total = (end - start).num_seconds() as f64;
    let projected_percent = percent * (total / elapsed).max(1.0);
    Some(Estimate {
        used,
        budget,
        percent,
        projected_percent,
        resets_at: end,
    })
}

pub fn window(e: &Estimate) -> UsageWindow {
    UsageWindow::approximate(
        WindowType::Monthly,
        u64::from(e.used),
        u64::from(e.budget),
        Some(e.resets_at),
        UsageUnit::Requests,
        format!(
            "Estimate: {} of ~{} prompts this cycle; at this pace ~{:.0}% by reset.",
            e.used, e.budget, e.projected_percent
        ),
    )
}

/// Budget that makes the estimate equal `actual_percent` right now.
pub fn calibrate(
    activity: &CursorActivity,
    actual_percent: f64,
    cycle_start_day: u8,
    now: DateTime<Utc>,
) -> Result<u32, String> {
    if !(0.5..=100.0).contains(&actual_percent) {
        return Err("Enter the percentage shown on cursor.com/dashboard (0.5–100).".into());
    }
    let (start, _) = cycle_bounds(now.with_timezone(&Local).date_naive(), cycle_start_day);
    let used = activity.prompts_since(start);
    if used == 0 {
        return Err(
            "No prompts recorded locally this cycle yet; send a few in Cursor, then calibrate."
                .into(),
        );
    }
    Ok(((f64::from(used) * 100.0 / actual_percent).round() as u32).max(used))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn activity_with(times: &[DateTime<Utc>]) -> CursorActivity {
        let mut a = CursorActivity::default();
        a.prompt_times = times.to_vec();
        a.prompt_times.sort();
        a
    }

    #[test]
    fn cycle_bounds_wrap_months() {
        let d = |y, m, day| NaiveDate::from_ymd_opt(y, m, day).unwrap();
        let local = |t: DateTime<Utc>| t.with_timezone(&Local).date_naive();
        let (s, e) = cycle_bounds(d(2026, 11, 5), 15);
        assert_eq!((local(s), local(e)), (d(2026, 10, 15), d(2026, 11, 15)));
        let (s, e) = cycle_bounds(d(2026, 11, 20), 15);
        assert_eq!((local(s), local(e)), (d(2026, 11, 15), d(2026, 12, 15)));
        let (s, _) = cycle_bounds(d(2026, 1, 3), 10);
        assert_eq!(local(s), d(2025, 12, 10));
    }

    #[test]
    fn calibration_reproduces_the_dashboard_percentage() {
        let now = Utc::now();
        let times: Vec<_> = (0..37).map(|i| now - Duration::minutes(i)).collect();
        let a = activity_with(&times);
        // A cycle that clearly started before the last 37 minutes.
        let day = if now.with_timezone(&Local).day() > 1 { 1 } else { 28 };
        let budget = calibrate(&a, 23.0, day, now).unwrap();
        assert_eq!(budget, 161); // 37 / 0.23
        let cfg = EstimateConfig { monthly_prompts: Some(budget), cycle_start_day: day };
        let e = estimate(&a, cfg, now).unwrap();
        assert!((e.percent - 23.0).abs() < 0.1, "{}", e.percent);
        assert!(e.projected_percent >= e.percent);
    }

    #[test]
    fn no_budget_no_estimate_and_bad_input_rejected() {
        let a = activity_with(&[Utc::now()]);
        assert!(estimate(&a, EstimateConfig::default(), Utc::now()).is_none());
        assert!(calibrate(&a, 0.0, 1, Utc::now()).is_err());
        assert!(calibrate(&CursorActivity::default(), 20.0, 1, Utc::now()).is_err());
    }
}
