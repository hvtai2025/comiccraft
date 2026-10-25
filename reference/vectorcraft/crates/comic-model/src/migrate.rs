//! §11 migrations: reading an older bundle forward.
//!
//! §2 splits the versions in two: a reader **refuses** a `schema_version` greater than the one it
//! implements ([`check_supported`](crate::check_supported)) and **accepts and migrates forward**
//! any lesser one. This module is the forward half, and it exists so that half is a *step* — with a
//! version pair, a name and a report — rather than a result nobody can point at.
//!
//! What §11 requires of a migration, and where each requirement is met:
//!
//! - **Validate its input and refuse rather than guess** — a version with no step implemented is an
//!   [`Error::Migration`], not a best effort.
//! - **Preserve every stable id**, including `retired_artboard_ids` — no step here reads or writes
//!   an id.
//! - **Report** what it did, visibly and actionably, rather than doing it silently — that is
//!   [`Migration`], collected by [`load`](crate::load) into
//!   [`LoadedBundle::migrations`](crate::LoadedBundle::migrations).
//! - **Preserve unknown fields** — no step touches [`Unknown`](crate::Unknown).
//! - **Ship with a fixture for every previous version** — `tests/fixtures/v1/` is the v1 anchor and
//!   `tests/fixture_v1.rs` loads it under v2 code (§11.1).
//!
//! # The v2 step, and what a field default does *not* cover
//!
//! v2 adds exactly one field, `Panel.locked` (§6.2), default `false`. A v1 file has no such key, so
//! the *value* is supplied at parse time by that field's `#[serde(default)]` — the reader half of
//! §5.7, "a reader must accept any valid JSON". A migration cannot be the thing that writes `false`
//! over a parsed model, because a `false` that came from the key and a `false` that came from the
//! key's absence are the same value by the time the model exists.
//!
//! So the step does what a `default` cannot:
//!
//! - it names the field v2 added and its default **in one place**, so the next version has a
//!   neighbouring step to imitate rather than a serde attribute to remember;
//! - it **counts** the panels that take the default, because "opening this project moved it
//!   forward" is something the creator is entitled to be told;
//! - it refuses to be silent about the one input a `default` would silently *accept*: a v1 bundle
//!   that **declares** `locked`. A v1 writer never wrote that key, so its presence is a hand-edit
//!   or a foreign tool; the value is preserved and reported rather than honoured in the dark.
//!
//! That last case has a stated limit: after parsing, `locked: false` is indistinguishable from the
//! key's absence, so only a truthy value is reportable. The limit is written here rather than
//! papered over — the alternative would be a second, hand-maintained shape of every entity.

use std::fmt;

use crate::SCHEMA_VERSION;
use crate::bundle::ComicBundle;
use crate::error::Error;

/// What one migration step did (§11's "report").
///
/// A step is a version pair plus the notes a caller can show or log. `notes` is never empty for a
/// step that ran: an unobservable migration would be one the creator could not be told about.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Migration {
    /// The version the bundle declared.
    pub from: u32,
    /// The version it was read as.
    pub to: u32,
    /// One actionable sentence per observable effect, in the step's own order.
    pub notes: Vec<String>,
}

impl fmt::Display for Migration {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "migrated bundle schema version {} to {}: {}", self.from, self.to, self.notes.join("; "))
    }
}

/// Migrate a freshly loaded bundle forward to [`SCHEMA_VERSION`] (§2, §11).
///
/// `Ok(None)` when the bundle already declares the version this build implements — a current bundle
/// is not a migration, and a report saying so would be noise on every load of every project.
///
/// The step runs on the **model**, after every file has parsed and the id counter has been
/// repaired, so a migration that refuses leaves the caller with nothing it could mistake for a
/// usable bundle.
pub fn migrate(bundle: &mut ComicBundle, from: u32) -> Result<Option<Migration>, Error> {
    match from {
        v if v == SCHEMA_VERSION => Ok(None),
        1 => Ok(Some(v1_to_v2(bundle))),
        other => Err(Error::Migration {
            found: other,
            supported: SCHEMA_VERSION,
            reason: format!("no migration from version {other} to {SCHEMA_VERSION} is implemented"),
        }),
    }
}

/// v1 → v2: `Panel.locked` (§6.2).
///
/// The field's default is applied by the reader (see the module docs); this step states the
/// transition and reports it, and it is what notices a v1 bundle that declares the key anyway.
fn v1_to_v2(bundle: &ComicBundle) -> Migration {
    let mut took_default = 0usize;
    let mut declared: Vec<String> = Vec::new();
    for (id, file) in &bundle.pages {
        for panel in &file.panels {
            if panel.locked {
                declared.push(format!(
                    "`pages/page-{}.json`: Panel({}) declares `locked: true`, which no v1 writer could have written",
                    id.0, panel.id.0
                ));
            } else {
                took_default += 1;
            }
        }
    }
    let plural = if took_default == 1 { "panel takes" } else { "panels take" };
    let mut notes = vec![format!("v2 adds `Panel.locked` (default `false`); {took_default} {plural} the default")];
    notes.extend(declared);
    Migration { from: 1, to: 2, notes }
}
