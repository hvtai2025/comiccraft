//! ComicCraft AI provider adapters: what a provider can do, the request we build for it, and
//! the state machine a generation job moves through.
//!
//! This crate knows about requests, responses and jobs. It knows **nothing** about documents,
//! rendering or the UI, and it never mutates project state: a job produces an asset and a
//! status, and only the `comic.job.apply` command writes (design §10.3).
//!
//! Two decisions from the design constrain everything here:
//!
//! - **Capabilities are declared, not assumed** (design §9.3). A provider describes which
//!   operations it supports, and a request is built from that description rather than from a
//!   guess that every model does everything.
//! - **Jobs are pollable, not push** (ADR-016). MCP accepts and ignores progress and
//!   cancellation notifications, so `comic.job.status` is the interface and progress is
//!   reported by reading a job, not by being notified.
//!
//! There is **no networking dependency in this crate yet**. Adding one is a deliberate,
//! gate-visible decision (ADR-015): it must be licence-compatible, gated so `cargo xtask wasm`
//! still passes, and unreachable from `crates/doc` and `crates/format`. Until then this crate
//! describes contracts and holds no transport.
#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};

/// A provider's identity, as recorded in every asset's provenance.
///
/// Recorded verbatim with the model name and the exact request, so a generated asset can
/// always be traced back to what produced it (design §9.3).
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ProviderId(pub String);

/// Where a generation job is in its life.
///
/// Frozen by `docs/comiccraft-project-schema.md` §8.1. Every transition is a value change on
/// the job record — never a document change.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JobStatus {
    /// Accepted, not yet sent to a provider.
    Queued,
    /// Sent; awaiting a response.
    Running,
    /// A result exists and is being validated.
    Completed,
    /// The provider or the transport failed.
    Failed,
    /// A result exists and is waiting for a human decision (design §9.4).
    AwaitingReview,
    /// Stopped locally, and remotely where the provider supports it.
    Cancelled,
}

impl JobStatus {
    /// Whether the job has stopped and will not change again without a new job.
    ///
    /// `AwaitingReview` is deliberately **not** terminal: the result exists but the job is
    /// still awaiting a decision, and `comic.job.apply` or `comic.job.reject` may still act
    /// on it.
    #[must_use]
    pub fn is_terminal(self) -> bool {
        matches!(self, Self::Failed | Self::Cancelled)
    }

    /// Whether a result asset is available to apply.
    #[must_use]
    pub fn has_result(self) -> bool {
        matches!(self, Self::Completed | Self::AwaitingReview)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn awaiting_review_is_not_terminal() {
        // A job holding a result a human has not yet accepted must stay actionable, or the
        // approval gate of design §9.4 could never be satisfied.
        assert!(!JobStatus::AwaitingReview.is_terminal());
        assert!(JobStatus::AwaitingReview.has_result());
    }

    #[test]
    fn only_failure_and_cancellation_are_terminal() {
        for status in [JobStatus::Queued, JobStatus::Running, JobStatus::Completed] {
            assert!(!status.is_terminal(), "{status:?} must not be terminal");
        }
        assert!(JobStatus::Failed.is_terminal());
        assert!(JobStatus::Cancelled.is_terminal());
    }

    #[test]
    fn terminal_states_never_carry_a_result() {
        // A failed or cancelled job must not offer an asset to apply.
        for status in [JobStatus::Failed, JobStatus::Cancelled] {
            assert!(!status.has_result(), "{status:?} must not report a result");
        }
    }

    #[test]
    fn status_serializes_as_snake_case() {
        // The wire names are fixed by the schema; a rename would break saved job records.
        assert_eq!(serde_json::to_string(&JobStatus::AwaitingReview).expect("serializes"), "\"awaiting_review\"");
        assert_eq!(serde_json::to_string(&JobStatus::Queued).expect("serializes"), "\"queued\"");
    }

    #[test]
    fn the_predicates_cover_every_state() {
        // Round 1 found the truth table incomplete: `has_result` was never checked for
        // `Queued` or `Running`, so a slip making a not-yet-started job report a result
        // would have passed.
        for status in
            [JobStatus::Queued, JobStatus::Running, JobStatus::Completed, JobStatus::Failed, JobStatus::AwaitingReview, JobStatus::Cancelled]
        {
            assert!(!(status.is_terminal() && status.has_result()), "{status:?} cannot be both stopped and actionable");
        }
        assert!(!JobStatus::Queued.has_result());
        assert!(!JobStatus::Running.has_result());
        assert!(!JobStatus::Completed.is_terminal());
        assert!(!JobStatus::AwaitingReview.is_terminal());
    }

    #[test]
    fn status_round_trips_and_refuses_unknown_names() {
        // Deserialization is the path a saved job record takes. A missing `rename_all` on
        // that side would load a bundle's jobs wrong, and a permissive fallback would turn a
        // newer bundle into a silently mis-read one (schema §2: refuse, never partially load).
        for status in
            [JobStatus::Queued, JobStatus::Running, JobStatus::Completed, JobStatus::Failed, JobStatus::AwaitingReview, JobStatus::Cancelled]
        {
            let json = serde_json::to_string(&status).expect("serializes");
            let back: JobStatus = serde_json::from_str(&json).expect("round trips");
            assert_eq!(back, status, "{status:?} via {json}");
        }
        for bad in [r#""queued_v2""#, r#""done""#, "null", "{}"] {
            assert!(serde_json::from_str::<JobStatus>(bad).is_err(), "{bad} must not become a JobStatus");
        }
    }

    #[test]
    fn provider_id_is_a_plain_string_on_the_wire() {
        // Provenance records a provider id verbatim; a wrapped object would break the
        // schema's `provider: ProviderId` field for any other reader.
        let json = serde_json::to_string(&ProviderId("example-provider".to_string())).expect("serializes");
        assert_eq!(json, "\"example-provider\"");
    }
}
