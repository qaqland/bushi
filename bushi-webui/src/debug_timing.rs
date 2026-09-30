use std::fmt::Write as _;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use axum::{
    extract::Request,
    http::{HeaderName, HeaderValue},
    middleware::Next,
    response::Response,
};

tokio::task_local! {
    static CONTEXT: TimingContext;
}

#[derive(Clone)]
struct TimingContext {
    inner: Arc<TimingInner>,
}

struct TimingInner {
    started: Instant,
    stages: Mutex<Vec<StageTiming>>,
}

#[derive(Clone)]
struct StageTiming {
    name: &'static str,
    total: Duration,
}

struct TimingSnapshot {
    total: Duration,
    stages: Vec<StageTiming>,
}

pub(crate) struct Timer {
    active: Option<(TimingContext, &'static str, Instant)>,
}

impl TimingContext {
    fn new() -> Self {
        Self {
            inner: Arc::new(TimingInner {
                started: Instant::now(),
                stages: Mutex::new(Vec::new()),
            }),
        }
    }

    fn record(&self, name: &'static str, elapsed: Duration) {
        let mut stages = self
            .inner
            .stages
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if let Some(stage) = stages.iter_mut().find(|stage| stage.name == name) {
            stage.total += elapsed;
        } else {
            stages.push(StageTiming {
                name,
                total: elapsed,
            });
        }
    }

    fn snapshot(&self) -> TimingSnapshot {
        let stages = self
            .inner
            .stages
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone();
        TimingSnapshot {
            total: self.inner.started.elapsed(),
            stages,
        }
    }
}

impl Drop for Timer {
    fn drop(&mut self) {
        if let Some((context, name, started)) = self.active.take() {
            context.record(name, started.elapsed());
        }
    }
}

const SERVER_TIMING: HeaderName = HeaderName::from_static("server-timing");

/// Times every request and reports the stages through the standard `Server-Timing`
/// response header, so browsers show them in DevTools without touching the body.
pub(crate) async fn scope_request(request: Request, next: Next) -> Response {
    CONTEXT
        .scope(TimingContext::new(), async {
            let mut response = next.run(request).await;
            if let Some(timing) = snapshot().map(|snapshot| server_timing(&snapshot))
                && let Ok(value) = HeaderValue::from_str(&timing)
            {
                response.headers_mut().insert(SERVER_TIMING, value);
            }
            response
        })
        .await
}

pub(crate) fn request_elapsed() -> Option<Duration> {
    CONTEXT
        .try_with(|context| context.inner.started.elapsed())
        .ok()
}

pub(crate) fn start(name: &'static str) -> Timer {
    let active = CONTEXT
        .try_with(|context| (context.clone(), name, Instant::now()))
        .ok();
    Timer { active }
}

fn snapshot() -> Option<TimingSnapshot> {
    CONTEXT.try_with(TimingContext::snapshot).ok()
}

fn server_timing(snapshot: &TimingSnapshot) -> String {
    let mut header = String::new();
    for stage in &snapshot.stages {
        if !header.is_empty() {
            header.push_str(", ");
        }
        header.extend(stage.name.chars().map(token_char));
        let _ = write!(header, ";dur={}", whole_millis(stage.total));
    }
    if !header.is_empty() {
        header.push_str(", ");
    }
    let _ = write!(header, "total;dur={}", whole_millis(snapshot.total));
    header
}

/// Whole milliseconds, matching the footer's precision. `as_millis` alone would
/// truncate, so a 0.9ms stage would read as 0 instead of 1.
fn whole_millis(duration: Duration) -> u128 {
    (duration + Duration::from_micros(500)).as_millis()
}

/// Metric names are RFC 7230 tokens; one stray character drops the whole metric.
fn token_char(c: char) -> char {
    if c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-') {
        c
    } else {
        '_'
    }
}

#[cfg(test)]
mod tests {
    use super::{StageTiming, TimingSnapshot, server_timing};
    use std::time::Duration;

    #[test]
    fn server_timing_rounds_to_whole_milliseconds() {
        let snapshot = TimingSnapshot {
            total: Duration::from_millis(12),
            stages: vec![
                StageTiming {
                    name: "render.body",
                    total: Duration::from_micros(1500),
                },
                StageTiming {
                    name: "sqlite pool wait",
                    total: Duration::from_millis(2),
                },
                StageTiming {
                    name: "cache.hit",
                    total: Duration::from_micros(400),
                },
            ],
        };
        assert_eq!(
            server_timing(&snapshot),
            "render.body;dur=2, sqlite_pool_wait;dur=2, cache.hit;dur=0, total;dur=12"
        );
        assert_eq!(
            server_timing(&TimingSnapshot {
                total: Duration::from_millis(3),
                stages: Vec::new(),
            }),
            "total;dur=3"
        );
    }
}
