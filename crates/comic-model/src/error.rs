//! Why a bundle could not be read or written.
//!
//! `docs/comiccraft-project-schema.md` §14 makes exactly two load failures fatal: the visual
//! document will not parse, and a comic file will not parse. The first is the caller's step,
//! because this crate sits at L2 and `crates/format` is L3 (§3's three path bases are the same
//! boundary); the second is [`Error::Parse`]. Everything else the loader finds — a `files`/tree
//! mismatch, a referential rule, an escaped path, a retired artboard — is a
//! [`Finding`](crate::Finding) on a usable project, not an error, so a creator can open a
//! damaged bundle and repair it rather than being locked out.
//!
//! So this type is deliberately narrow: I/O, a parse failure, and the two version refusals §2
//! requires.

use std::path::PathBuf;

/// Why a bundle could not be read or written.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// A bundle file could not be read.
    #[error("cannot read `{path}`: {source}")]
    Read {
        /// The file.
        path: PathBuf,
        /// The underlying failure.
        #[source]
        source: std::io::Error,
    },
    /// A bundle file could not be written.
    #[error("cannot write `{path}`: {source}")]
    Write {
        /// The file.
        path: PathBuf,
        /// The underlying failure.
        #[source]
        source: std::io::Error,
    },
    /// A bundle file is not valid JSON, or is valid JSON of the wrong shape. Fatal (§14 step 3).
    #[error("`{path}` is not a ComicCraft `{what}`: {source}")]
    Parse {
        /// The file.
        path: PathBuf,
        /// What the file was expected to hold.
        what: &'static str,
        /// The underlying failure.
        #[source]
        source: serde_json::Error,
    },
    /// A value could not be written as canonical JSON (§5.7).
    #[error("cannot serialize `{what}` as canonical JSON: {source}")]
    Serialize {
        /// What was being written.
        what: String,
        /// The underlying failure.
        #[source]
        source: serde_json::Error,
    },
    /// The directory holds no `manifest.json`, so it is not a bundle (§3: a bundle is
    /// identified by its manifest).
    #[error("`{path}` is not a ComicCraft bundle: `manifest.json` is missing")]
    MissingManifest {
        /// The directory that was opened.
        path: PathBuf,
    },
    /// The manifest declares a schema version this build does not implement. §2: refuse, naming
    /// the required version; never partially load.
    #[error(
        "bundle schema version {found} is newer than the {supported} this build implements: \
         upgrade ComicCraft to open this project"
    )]
    SchemaTooNew {
        /// The version in `manifest.json`.
        found: u32,
        /// [`SCHEMA_VERSION`](crate::SCHEMA_VERSION).
        supported: u32,
    },
    /// A migration refused rather than guessed (§11).
    #[error("cannot migrate bundle schema version {found} to {supported}: {reason}")]
    Migration {
        /// The version found.
        found: u32,
        /// The version this build implements.
        supported: u32,
        /// Why the migration refused.
        reason: String,
    },
    /// A **file** inside the bundle declares a schema version this build does not implement. §2:
    /// refuse, naming the required version; never partially load. The manifest's own copy of the
    /// version is [`Error::SchemaTooNew`]'s; this is the same refusal for a file that declares one
    /// of its own, which rule 3 governs when it is *older* and §2 refuses when it is newer.
    #[error(
        "`{path}` declares comic bundle schema version {found}, newer than the {supported} this build implements: upgrade ComicCraft to open this project"
    )]
    FileSchemaTooNew {
        /// The file that declares it.
        path: PathBuf,
        /// The version the file declares.
        found: u32,
        /// [`SCHEMA_VERSION`](crate::SCHEMA_VERSION).
        supported: u32,
    },
    /// A file the bundle needs is not there.
    #[error("`{path}` does not exist")]
    Missing {
        /// The file.
        path: PathBuf,
    },
    /// A path in the bundle is not valid UTF-8, so it cannot be a bundle-relative `/`-separated
    /// key (§3).
    #[error("`{path}` is not valid UTF-8, so it cannot be a bundle-relative path")]
    NonUtf8Path {
        /// The offending path.
        path: PathBuf,
    },
    /// A bundle-relative path is not one: it is absolute, empty, or walks up with `..`.
    #[error("`{path}` is not a bundle-relative path (no leading `/`, no `..`, no empty component)")]
    BadBundlePath {
        /// The path found.
        path: String,
    },
    /// A path cannot be expressed relative to the bundle root.
    #[error("`{path}` is not inside the bundle root `{root}`")]
    OutsideRoot {
        /// The path.
        path: PathBuf,
        /// The root it should have been under.
        root: PathBuf,
    },
    /// A timestamp is not in the one canonical form §5.7 and §5.5 fix, so a save could not be
    /// byte-identical. The writer refuses rather than guesses: repairing `2026-10-11 06:00:00`
    /// would mean assuming a timezone nobody recorded.
    ///
    /// **Only the writer refuses it.** `load` accepts a bundle whose timestamps are not canonical,
    /// because §5.7 makes producing the form the writer's duty and §14 keeps a damaged bundle
    /// openable; a caller that wants to warn the creator early asks
    /// [`ComicBundle::non_canonical_timestamps`](crate::ComicBundle::non_canonical_timestamps),
    /// which returns this same list.
    #[error("`{field}` is `{value}`, not an RFC 3339 UTC second-precision timestamp (e.g. `2026-10-10T21:52:00Z`)")]
    BadTimestamp {
        /// The field or option that holds it.
        field: String,
        /// The value.
        value: String,
    },
    /// The id counter reached the end of `u64`, so the next id would repeat one already issued
    /// (§9: an id is never reused).
    #[error("the comic id counter is exhausted at {next}: no id can be issued without reusing one")]
    IdsExhausted {
        /// The counter's value.
        next: u64,
    },
}
