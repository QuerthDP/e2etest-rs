/*
 * Copyright 2026-present ScyllaDB
 * SPDX-License-Identifier: MIT OR Apache-2.0
 */

use std::fmt::Debug;
use std::ops::Add;
use std::ops::AddAssign;
use std::sync::Arc;
use std::sync::Mutex;

#[derive(Clone)]
/// Statistics for a test run, including total tests, launched, successful, and failed.
pub struct Statistics(Arc<Mutex<Inner>>);

impl Debug for Statistics {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Statistics")
            .field("total", &self.total())
            .field("included", &self.included())
            .field("launched", &self.launched())
            .field("skipped", &self.skipped())
            .field("skipped_groups", &self.skipped_groups())
            .field("ok", &self.ok())
            .field("failed_tests", &self.failed_tests())
            .field("failed_groups", &self.failed_groups())
            .finish()
    }
}

struct Inner {
    total: usize,
    included: usize,
    launched: usize,
    skipped: usize,
    skipped_groups: usize,
    ok: usize,
    failed_tests: usize,
    failed_groups: usize,
    failed_names: Vec<String>,
}

impl Inner {
    fn new() -> Self {
        Self {
            total: 0,
            included: 0,
            launched: 0,
            skipped: 0,
            skipped_groups: 0,
            ok: 0,
            failed_tests: 0,
            failed_groups: 0,
            failed_names: Vec::new(),
        }
    }

    fn append(&mut self, other: &Self) {
        self.total += other.total;
        self.included += other.included;
        self.launched += other.launched;
        self.skipped += other.skipped;
        self.skipped_groups += other.skipped_groups;
        self.ok += other.ok;
        self.failed_tests += other.failed_tests;
        self.failed_groups += other.failed_groups;
        self.failed_names.extend(other.failed_names.iter().cloned());
    }
}

impl Add for Statistics {
    type Output = Self;

    fn add(self, other: Self) -> Self {
        self + &other
    }
}

impl Add<&Statistics> for Statistics {
    type Output = Self;

    fn add(mut self, other: &Self) -> Self {
        self += other;
        self
    }
}

impl AddAssign for Statistics {
    fn add_assign(&mut self, other: Self) {
        *self += &other;
    }
}

impl AddAssign<&Statistics> for Statistics {
    fn add_assign(&mut self, other: &Self) {
        let mut inner = self.0.lock().unwrap();
        let other_inner = other.0.lock().unwrap();
        inner.append(&other_inner);
    }
}

impl Statistics {
    pub(crate) fn new() -> Self {
        Self(Arc::new(Mutex::new(Inner::new())))
    }

    pub(crate) fn increment_total(&self, count: usize) {
        let mut inner = self.0.lock().unwrap();
        inner.total += count;
    }

    pub(crate) fn increment_included(&self, count: usize) {
        let mut inner = self.0.lock().unwrap();
        inner.included += count;
    }

    pub(crate) fn increment_launched(&self) {
        let mut inner = self.0.lock().unwrap();
        inner.launched += 1;
    }

    pub(crate) fn increment_ok(&self) {
        let mut inner = self.0.lock().unwrap();
        inner.ok += 1;
    }

    pub(crate) fn record_test_failure(&self, failed_test: impl Into<String>) {
        let mut inner = self.0.lock().unwrap();
        inner.failed_tests += 1;
        inner.failed_names.push(failed_test.into());
    }

    pub(crate) fn record_group_failure(&self, failed_group: impl Into<String>) {
        let mut inner = self.0.lock().unwrap();
        inner.failed_groups += 1;
        inner.failed_names.push(failed_group.into());
    }

    pub(crate) fn increment_skipped(&self) {
        let mut inner = self.0.lock().unwrap();
        inner.skipped += 1;
    }

    pub(crate) fn increment_skipped_groups(&self) {
        let mut inner = self.0.lock().unwrap();
        inner.skipped_groups += 1;
    }

    /// Returns true if there are no failed tests or groups.
    pub fn is_success(&self) -> bool {
        let inner = self.0.lock().unwrap();
        inner.failed_names.is_empty()
    }

    /// Returns number of total defined tests.
    pub fn total(&self) -> usize {
        let inner = self.0.lock().unwrap();
        inner.total
    }

    /// Returns number of tests included in the run after filtering.
    pub fn included(&self) -> usize {
        let inner = self.0.lock().unwrap();
        inner.included
    }

    /// Returns number of tests that were launched.
    pub fn launched(&self) -> usize {
        let inner = self.0.lock().unwrap();
        inner.launched
    }

    /// Returns number of tests that passed successfully.
    pub fn ok(&self) -> usize {
        let inner = self.0.lock().unwrap();
        inner.ok
    }

    /// Returns number of tests that failed.
    pub fn failed_tests(&self) -> usize {
        let inner = self.0.lock().unwrap();
        inner.failed_tests
    }

    /// Returns number of groups that failed.
    pub fn failed_groups(&self) -> usize {
        let inner = self.0.lock().unwrap();
        inner.failed_groups
    }

    /// Returns number of tests that were skipped.
    pub fn skipped(&self) -> usize {
        let inner = self.0.lock().unwrap();
        inner.skipped
    }

    /// Returns number of groups that were skipped.
    pub fn skipped_groups(&self) -> usize {
        let inner = self.0.lock().unwrap();
        inner.skipped_groups
    }

    /// Returns a list of names of failed tests and groups.
    pub fn failed_names(&self) -> Vec<String> {
        let inner = self.0.lock().unwrap();
        inner.failed_names.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add() {
        let stats_base = Statistics::new();
        stats_base.increment_total(2);
        stats_base.increment_launched();
        stats_base.increment_skipped();
        stats_base.increment_skipped_groups();
        stats_base.increment_ok();
        stats_base.record_test_failure("crud::boom");
        stats_base.record_group_failure("foo");

        let stats = Statistics::new();
        stats.increment_total(20);
        stats.increment_launched();
        stats.increment_skipped();
        stats.increment_skipped_groups();
        stats.increment_ok();
        stats.record_test_failure("crud::cleanup");
        stats.record_group_failure("boo");

        let stats = stats_base + stats;

        assert_eq!(stats.total(), 22);
        assert_eq!(stats.launched(), 2);
        assert_eq!(stats.skipped(), 2);
        assert_eq!(stats.skipped_groups(), 2);
        assert_eq!(stats.ok(), 2);
        assert_eq!(stats.failed_tests(), 2);
        assert_eq!(stats.failed_groups(), 2);
        assert_eq!(
            stats.failed_names(),
            vec![
                "crud::boom".to_string(),
                "foo".to_string(),
                "crud::cleanup".to_string(),
                "boo".to_string()
            ]
        );
    }
}
