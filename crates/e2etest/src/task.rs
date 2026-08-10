/*
 * Copyright 2026-present ScyllaDB
 * SPDX-License-Identifier: MIT OR Apache-2.0
 */

use crate::run::RunContext;
use crate::statistics::Event;
use crate::statistics::Task;
use async_backtrace::frame;
use async_backtrace::framed;
use std::time::Duration;
use tokio::time;
use tracing::Instrument;
use tracing::Span;
use tracing::error;
use tracing::error_span;
use tracing::info;

#[framed]
pub(crate) async fn setup<T: Send + Sync + 'static>(
    name: &str,
    task: Task,
    setup: impl Future<Output = Option<T>> + Send + 'static,
    timeout: Duration,
    ctx: RunContext,
) -> Result<Option<T>, ()> {
    let span = error_span!("setup");
    ctx.statistics.record(name, Event::SetupLaunched);
    let result = single(&span, setup, timeout, ctx.clone()).await;
    match &result {
        Ok(Some(_)) => {
            info!(parent: &span, "passed");
            ctx.statistics.record(name, Event::SetupPassed);
        }
        Ok(None) => {
            info!(parent: &span, "skipped");
            ctx.statistics.record(name, Event::SetupSkipped(task))
        }
        Err(_) => ctx.statistics.record(name, Event::SetupFailed(task)),
    }
    result
}

#[framed]
pub(crate) async fn teardown(
    name: &str,
    teardown: impl Future<Output = ()> + Send + 'static,
    timeout: Duration,
    ctx: RunContext,
) {
    let span = error_span!("teardown");
    ctx.statistics.record(name, Event::TeardownLaunched);
    let result = single::<()>(&span, teardown, timeout, ctx.clone()).await;
    if result.is_ok() {
        info!(parent: &span, "passed");
        ctx.statistics.record(name, Event::TeardownPassed);
    } else {
        ctx.statistics.record(name, Event::TeardownFailed);
    }
}

#[framed]
pub(crate) async fn test(
    name: &str,
    run: impl Future<Output = ()> + Send + 'static,
    timeout: Duration,
    ctx: RunContext,
) {
    let span = error_span!("run");
    ctx.statistics.record(name, Event::TestLaunched);
    let result = single::<()>(&span, run, timeout, ctx.clone()).await;
    if result.is_ok() {
        info!(parent: &span, "passed");
        ctx.statistics.record(name, Event::TestPassed);
    } else {
        ctx.statistics.record(name, Event::TestFailed);
    }
}

#[framed]
pub(crate) async fn single<T: Send + Sync + 'static>(
    span: &Span,
    fut: impl Future<Output = T> + Send + 'static,
    timeout: Duration,
    ctx: RunContext,
) -> Result<T, ()> {
    let task_result = tokio::spawn(frame!(
        async move { time::timeout(timeout, fut).await.expect("timed out") }
            .instrument(span.clone())
    ))
    .await;

    match task_result {
        Err(err) => {
            let backtrace = ctx.backtrace.get();
            error!(parent: span, "failed: {err}\n{backtrace}");
            Err(())
        }
        Ok(t) => Ok(t),
    }
}
