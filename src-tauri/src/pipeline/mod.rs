//! The job-fetch pipeline: pure filtering (`filter`), the durable queue + runner, and the
//! discovery chain. Built across Phase A tasks 2–6.

pub mod filter;
pub mod queue;
pub mod runner;
