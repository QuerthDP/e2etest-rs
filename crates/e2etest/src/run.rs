/*
 * Copyright 2026-present ScyllaDB
 * SPDX-License-Identifier: MIT OR Apache-2.0
 */

use crate::DEFAULT_TIMEOUT;
use crate::backtrace;
use crate::backtrace::Backtrace;
use crate::filter::Filter;
use crate::fixture::Fixtures;
use crate::group::RunGroup;
use crate::statistics::Statistics;
use async_backtrace::framed;
use itertools::Itertools;
use std::fmt::Debug;
use std::iter;
use std::time::Duration;
use tracing::Instrument;
use tracing::error;
use tracing::error_span;
use tracing::info;

#[derive(Clone)]
pub struct RunContext {
    pub(crate) fixtures: Fixtures,
    pub(crate) statistics: Statistics,
    pub(crate) backtrace: Backtrace,
    pub(crate) filter: Filter,
    pub(crate) default_timeout: Duration,
    pub(crate) concurrency: usize,
    pub(crate) concurrency_enabled: bool,
}

impl RunContext {
    pub(crate) fn new() -> Self {
        Self {
            fixtures: Fixtures::new(),
            statistics: Statistics::new(),
            backtrace: Backtrace::new(),
            filter: Filter::empty(),
            default_timeout: DEFAULT_TIMEOUT,
            concurrency: 1,
            concurrency_enabled: true,
        }
    }

    pub(crate) fn with_fixtures(mut self, fixtures: Fixtures) -> Self {
        self.fixtures = fixtures;
        self
    }

    pub(crate) fn with_backtrace(mut self, backtrace: Backtrace) -> Self {
        self.backtrace = backtrace;
        self
    }

    pub(crate) fn with_filter(mut self, filter: Filter) -> Self {
        self.filter = filter;
        self
    }

    pub(crate) fn with_default_timeout(mut self, default_timeout: Duration) -> Self {
        self.default_timeout = default_timeout;
        self
    }

    pub(crate) fn with_concurrency(mut self, concurrency: usize) -> Self {
        self.concurrency = if concurrency > 0 { concurrency } else { 1 };
        self
    }

    pub(crate) fn update_concurrency_enabled(&mut self, enabled_for_next_level: bool) {
        self.concurrency_enabled &= enabled_for_next_level;
    }

    pub(crate) fn is_concurrency_enabled(&self, enabled_for_next_level: bool) -> bool {
        self.concurrency_enabled & enabled_for_next_level
    }
}

impl Debug for RunContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Run").finish()
    }
}

#[framed]
/// Runs all test cases, filtering them based on the provided filter map.
pub(crate) async fn run(
    fixtures: Fixtures,
    group: Box<dyn RunGroup>,
    filter: Filter,
    default_timeout: Duration,
    concurrency: usize,
) -> Statistics {
    let ctx = RunContext::new()
        .with_fixtures(fixtures)
        .with_filter(filter)
        .with_backtrace(backtrace::setup_panic_hook())
        .with_default_timeout(default_timeout)
        .with_concurrency(concurrency);

    ctx.statistics.increment_total(group.test_names().len());
    ctx.statistics.increment_included(
        group
            .test_names()
            .iter()
            .map(|name| name.split("::").collect_vec())
            .filter(|parts| {
                ctx.filter.consider_test(
                    iter::once(&group.name()).chain(&parts[..parts.len() - 1]),
                    parts.last().unwrap_or(&""),
                )
            })
            .count(),
    );

    group
        .run_group(vec![], ctx.clone())
        .instrument(error_span!("group", "{}", group.name()))
        .await;

    backtrace::clear_panic_hook();

    let stats = ctx.statistics;
    if stats.is_success() {
        info!("test run ok: {stats:?}");
    } else {
        error!("test run failed: {stats:?}");
    }
    stats
}
