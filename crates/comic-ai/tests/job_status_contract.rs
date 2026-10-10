//! Reviewer tests (`code-reviewer`, M0 round 1).
//!
//! The implementer asserted two serializations and no deserialization at all, and asserted
//! the wire names against literals written beside them. These tests pin the wire contract to
//! the frozen schema document itself, and complete the `JobStatus` truth table.

// `clippy.toml` sets `allow-panic-in-tests`, but clippy only applies it to `#[cfg(test)]`
// modules, not to an integration-test crate. The repository's policy (clippy.toml header) is
// that tests may panic, so this is the file-level equivalent for an integration test.
#![allow(clippy::panic, clippy::expect_used, clippy::unwrap_used)]

use std::collections::BTreeSet;
use std::fs;

use vectorcraft_comic_ai::{JobStatus, ProviderId};

const SCHEMA_DOC: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../docs/comiccraft-project-schema.md");

const ALL: [JobStatus; 6] =
    [JobStatus::Queued, JobStatus::Running, JobStatus::Completed, JobStatus::Failed, JobStatus::AwaitingReview, JobStatus::Cancelled];

/// The six `JobStatus` wire names as `docs/comiccraft-project-schema.md` §8.1 freezes them.
fn schema_job_status_names() -> BTreeSet<String> {
    let doc = fs::read_to_string(SCHEMA_DOC).unwrap_or_else(|e| panic!("the frozen schema doc must be readable: {e}"));
    // §8.1 wraps this line, so the parenthesised list may begin on the following line:
    //   `status: JobStatus`
    //   (`queued` | `running` | `completed` | `failed` | `awaiting_review` | `cancelled`),
    let at = doc.find("`status: JobStatus`").unwrap_or_else(|| panic!("schema §8.1 no longer states `status: JobStatus` with its wire names"));
    let tail = &doc[at + "`status: JobStatus`".len()..];
    let open = tail.find('(').unwrap_or_else(|| panic!("schema §8.1 no longer lists JobStatus states in parentheses after the key"));
    let close = tail[open..].find(')').unwrap_or_else(|| panic!("the JobStatus state list in schema §8.1 is unterminated"));
    let parsed: BTreeSet<String> = tail[open + 1..open + close]
        .split('|')
        // The document is markdown, so each name arrives wrapped in backticks and followed
        // by an escaped pipe (`queued` \|). Wire names are snake_case, so keeping only
        // `[a-z0-9_]` is unambiguous and survives re-wrapping of the prose.
        .map(|s| s.chars().filter(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || *c == '_').collect::<String>())
        .filter(|s| !s.is_empty())
        .collect();
    assert_eq!(parsed.len(), 6, "expected the six frozen JobStatus names, parsed {parsed:?}");
    parsed
}

#[test]
fn wire_names_match_the_frozen_schema_exactly() {
    let actual: BTreeSet<String> =
        ALL.iter().map(|s| serde_json::to_string(s).expect("serializes")).map(|s| s.trim_matches('"').to_string()).collect();
    assert_eq!(actual, schema_job_status_names(), "the crate's JobStatus wire names have drifted from schema §8.1");
}

#[test]
fn every_status_round_trips() {
    // The implementer tested serialization only; a missing `rename_all` on the Deserialize
    // side would load a saved job record wrong, and no test would notice.
    for status in ALL {
        let json = serde_json::to_string(&status).expect("serializes");
        let back: JobStatus = serde_json::from_str(&json).unwrap_or_else(|e| panic!("{status:?} does not round-trip via {json}: {e}"));
        assert_eq!(back, status);
    }
}

#[test]
fn an_unknown_status_is_refused_rather_than_defaulted() {
    // Schema §2: "Refuse ... with an actionable error. Never partially load." A
    // `#[serde(other)]` fallback, or an `unwrap_or_default` over a job status, would turn a
    // newer bundle into a silently mis-read one.
    for json in [r#""queued_v2""#, r#""done""#, r#""pending""#, "null", "{}"] {
        assert!(serde_json::from_str::<JobStatus>(json).is_err(), "{json} must not deserialize to a JobStatus");
    }
}

#[test]
fn the_predicates_partition_every_status() {
    // Complete truth table. The implementer asserted `is_terminal` for all six states but
    // `has_result` for only three, so `{Queued, Running}.has_result()` was never checked.
    let with_result: Vec<JobStatus> = ALL.into_iter().filter(|s| s.has_result()).collect();
    let terminal: Vec<JobStatus> = ALL.into_iter().filter(|s| s.is_terminal()).collect();
    assert_eq!(with_result, vec![JobStatus::Completed, JobStatus::AwaitingReview]);
    assert_eq!(terminal, vec![JobStatus::Failed, JobStatus::Cancelled]);

    for status in ALL {
        assert!(!(status.is_terminal() && status.has_result()), "{status:?} cannot be both stopped and actionable");
    }
    let pending_no_result: Vec<JobStatus> = ALL.into_iter().filter(|s| !s.is_terminal() && !s.has_result()).collect();
    assert_eq!(pending_no_result, vec![JobStatus::Queued, JobStatus::Running], "a job with nothing yet is neither terminal nor result-bearing");
}

// -------------------------------------------------------------------------------------------------
// Round 2 — failure paths and edges on the post-fix revision. Round 1 asked for "an unknown
// status is refused"; round 2 asks *how* it is refused: case, whitespace, empty input and the
// wrong JSON type are all the ways a bundle written by another tool goes wrong, and each of
// them must fail loudly (schema §2: refuse with an actionable error, never partially load).
// -------------------------------------------------------------------------------------------------

#[test]
fn an_unknown_status_name_is_refused_case_sensitively_and_without_trimming() {
    // A lenient matcher that lower-cased or trimmed would load a record written under a
    // different (or newer) convention as though it were the frozen one. The tab case is a JSON
    // *syntax* error rather than an unknown variant — it is refused too, but by the parser, so
    // it is asserted separately below.
    for bad in [
        r#""QUEUED""#,
        r#""Queued""#,
        r#"" queued""#,
        r#""queued ""#,
        r#""""#,
        r#""awaitingreview""#,
        r#""awaiting-review""#,
        r#""queued_""#,
        r#""__queued""#,
    ] {
        let err = serde_json::from_str::<JobStatus>(bad)
            .err()
            .unwrap_or_else(|| panic!("{bad} deserialized into a JobStatus; schema §2 forbids a silent coercion"));
        // §12.1 wants an actionable message, so a *semantic* refusal must name the variants the
        // schema froze — that is what tells a reader their file is wrong rather than the crate's.
        let message = err.to_string();
        for name in ["queued", "running", "completed", "failed", "awaiting_review", "cancelled"] {
            assert!(message.contains(name), "the refusal for {bad} must list `{name}`: {message}");
        }
    }
    // Malformed JSON is refused by the parser before the type is ever consulted. Still a
    // refusal, and still a loud one — but with a parse message, so it is asserted on its own.
    assert!(serde_json::from_str::<JobStatus>("\"\tqueued\"").is_err(), "a raw tab is a JSON syntax error");
    assert!(serde_json::from_str::<JobStatus>(r#""queued"#).is_err(), "a truncated string is refused");
    assert!(serde_json::from_str::<JobStatus>(r#""queued""#).is_ok(), "the frozen name itself still loads");
}

#[test]
fn a_status_is_not_built_from_anything_that_is_not_a_string() {
    // Wrong JSON type and empty input: `null`, an array, a number and an object are all
    // "something else", and each must be refused rather than defaulted.
    for bad in ["null", "0", "1", "[]", r#"["queued"]"#, "{}", r#"{"status":"queued"}"#, ""] {
        assert!(serde_json::from_str::<JobStatus>(bad).is_err(), "{bad} must not deserialize to a JobStatus");
    }
}

#[test]
fn provider_id_round_trips_and_refuses_anything_that_is_not_a_string() {
    // Round 1 pinned `ProviderId`'s *serialization* only. Its `Deserialize` half was untested,
    // and it is the half a provenance record takes when a bundle is read back: a bare number
    // or a nested object there would attribute a generated asset to nothing.
    for raw in ["openai", "", "a provider/with spaces", "üñïçø∂é", "with\"quote", "0123"] {
        let id = ProviderId(raw.to_string());
        let json = serde_json::to_string(&id).expect("ProviderId serializes");
        assert_eq!(json, serde_json::to_string(raw).expect("a JSON string serializes"), "{raw} must be a bare string");
        assert_eq!(serde_json::from_str::<ProviderId>(&json).expect("ProviderId deserializes"), id);
    }
    for bad in [r#"{"a":1}"#, "7", "null", "true", "[]", r#"["p"]"#] {
        assert!(serde_json::from_str::<ProviderId>(bad).is_err(), "{bad} must not become a ProviderId");
    }
}

#[test]
fn the_predicates_are_pure_functions_of_the_status() {
    // `is_terminal`/`has_result` take `self` by value on a `Copy` type and hold no state, so a
    // second call must agree — the poll loop of ADR-016 will call them repeatedly on one job.
    for status in ALL {
        let first = (status.is_terminal(), status.has_result());
        let second = (status.is_terminal(), status.has_result());
        assert_eq!(first, second, "{status:?} is not stable across calls");
        // A deserialized copy must judge identically to the value it came from.
        let json = serde_json::to_string(&status).expect("serializes");
        let back: JobStatus = serde_json::from_str(&json).expect("deserializes");
        assert_eq!((back.is_terminal(), back.has_result()), first, "{status:?} changed meaning across the wire");
    }
}
