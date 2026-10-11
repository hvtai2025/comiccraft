//! Canonical JSON (§5.7), content hashing (§7.2) and the bundle's file tree (§4, §14).
//!
//! §11.1 requires the v1 fixture to round-trip **byte-identically**, which is only meaningful if
//! exactly one byte sequence is canonical. §5.7 fixes it:
//!
//! - object keys in the order the struct declares them (serde's default), **not** sorted;
//! - two-space indentation, `\n` line endings, a trailing newline;
//! - numbers in the shortest form that round-trips — an integral type writes `1`, an `f64`
//!   writes `612.0`;
//! - map-valued fields (`unknown`, `provenance.request`, `manifest.files`) are the exception:
//!   their keys are written **sorted**, because `serde_json` sorts a `Map` and a fixture author
//!   cannot otherwise know what order a map with two keys takes.
//!
//! A struct field's order is declared; a map's is sorted. Both are canonical. `serde_json`'s
//! `Map` is a `BTreeMap` unless the `preserve_order` feature is on, and the workspace does not
//! enable it, which is what makes the sorted half true without any code here.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Component, Path, PathBuf};

use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::entities::{FileEntry, Unknown};
use crate::error::Error;

/// The bytes a writer must produce for one bundle file (§5.7).
pub fn to_canonical_bytes<T: Serialize>(what: &str, value: &T) -> Result<Vec<u8>, Error> {
    let mut text = serde_json::to_string_pretty(value).map_err(|e| Error::Serialize { what: what.to_string(), source: e })?;
    text.push('\n');
    Ok(text.into_bytes())
}

/// Lowercase hex sha256 of `bytes` (§7.2).
#[must_use]
pub fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hasher.finalize().iter().map(|b| format!("{b:02x}")).collect()
}

/// Every file in the bundle as §4 defines it, keyed bundle-relative with `/` separators:
/// everything **except** `manifest.json`, `exports/` and `.save-tmp/`.
///
/// Those three exceptions are the point. `exports/` and `.save-tmp/` are never read as input, so
/// listing them would make deleting `exports/` break every load; the manifest is the file that
/// declares the others, so it cannot hash itself.
///
/// Symbolic links are skipped rather than followed: a link inside `assets/` that points outside
/// the bundle must not become a file the manifest vouches for (§13.1).
pub fn scan_tree(root: &Path) -> Result<BTreeMap<String, FileEntry>, Error> {
    let mut out = BTreeMap::new();
    walk(root, root, &mut out)?;
    Ok(out)
}

fn walk(root: &Path, dir: &Path, out: &mut BTreeMap<String, FileEntry>) -> Result<(), Error> {
    let read = fs::read_dir(dir).map_err(|source| Error::Read { path: dir.to_path_buf(), source })?;
    for entry in read {
        let entry = entry.map_err(|source| Error::Read { path: dir.to_path_buf(), source })?;
        let path = entry.path();
        let kind = entry.file_type().map_err(|source| Error::Read { path: path.clone(), source })?;
        if kind.is_dir() {
            let name = entry.file_name();
            if dir == root && (name == "exports" || name == ".save-tmp") {
                continue;
            }
            walk(root, &path, out)?;
        } else if kind.is_file() {
            let key = relative_key(root, &path)?;
            if key == "manifest.json" {
                continue;
            }
            let bytes = fs::read(&path).map_err(|source| Error::Read { path: path.clone(), source })?;
            let bytes_len = bytes.len() as u64;
            out.insert(key, FileEntry { sha256: sha256_hex(&bytes), bytes: bytes_len, unknown: Unknown::new() });
        }
    }
    Ok(())
}

/// `path` as a bundle-relative `/`-separated key (§3).
pub fn relative_key(root: &Path, path: &Path) -> Result<String, Error> {
    let rel = path.strip_prefix(root).map_err(|_| Error::OutsideRoot { path: path.to_path_buf(), root: root.to_path_buf() })?;
    let mut parts = Vec::new();
    for component in rel.components() {
        match component {
            Component::Normal(name) => {
                let name = name.to_str().ok_or_else(|| Error::NonUtf8Path { path: path.to_path_buf() })?;
                parts.push(name.to_string());
            }
            _ => return Err(Error::OutsideRoot { path: path.to_path_buf(), root: root.to_path_buf() }),
        }
    }
    if parts.is_empty() {
        return Err(Error::OutsideRoot { path: path.to_path_buf(), root: root.to_path_buf() });
    }
    Ok(parts.join("/"))
}

/// A bundle-relative `/`-separated path as a relative [`PathBuf`], refusing to leave the base it
/// will be joined to.
///
/// The writer uses this as well as the reader: a `relative_path` that names its own root, walks up
/// with `..`, or contains an empty component is refused before anything is written, so a
/// hand-edited model cannot make a save write outside the bundle (§13 rule 9).
pub fn join_relative(relative: &str) -> Result<PathBuf, Error> {
    let mut out = PathBuf::new();
    for part in relative.split('/') {
        if part.is_empty() || part == "." || part == ".." {
            return Err(Error::BadBundlePath { path: relative.to_string() });
        }
        out.push(part);
    }
    Ok(out)
}

/// What resolving a bundle path produced.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PathResolution {
    /// The path is inside its base and exists.
    Resolved(PathBuf),
    /// The path leaves its base and is **never resolved** (§13 rule 9).
    Escapes,
    /// The path stays inside its base but nothing is there.
    Missing,
}

/// Resolve `relative` against `base`, canonicalising before the containment check (§13 rule 9).
///
/// The order is the rule, not an implementation detail: join, **canonicalise** — which resolves
/// both `..` and every symlink — and only then check containment. A `..` denylist would not do,
/// because `a/../..` escapes without a `..` prefix and a symlink escapes with no `..` at all, and
/// neither survives canonicalisation. An absolute `relative` is refused outright: every base in
/// the schema is a container, and a path that names its own root has already left it.
///
/// [`PathResolution::Escapes`] is deliberately distinct from [`PathResolution::Missing`]: a path
/// that escapes is a security finding, and a path that is merely absent is a broken link. The
/// two are told apart by canonicalising the nearest ancestor that exists, so an escaping path is
/// still reported as an escape when its target is not there.
///
/// Canonicalising and then opening leaves the TOCTOU window §13.1 records rather than hides; the
/// bundle is locally trusted, and integrity rests on `content_hash` (§7.2).
#[must_use]
pub fn resolve_path(base: &Path, relative: &str) -> PathResolution {
    if relative.is_empty() {
        return PathResolution::Escapes;
    }
    let named = Path::new(relative);
    if named.is_absolute() {
        return PathResolution::Escapes;
    }
    let Ok(base) = base.canonicalize() else { return PathResolution::Escapes };
    let joined = base.join(named);
    let mut probe = joined.as_path();
    loop {
        if let Ok(canonical) = probe.canonicalize() {
            return if canonical.starts_with(&base) {
                if probe == joined { PathResolution::Resolved(canonical) } else { PathResolution::Missing }
            } else {
                PathResolution::Escapes
            };
        }
        match probe.parent() {
            Some(parent) if parent != probe => probe = parent,
            _ => return PathResolution::Escapes,
        }
    }
}

/// [`resolve_path`], as an `Option`: `Some` only when the path is inside its base and exists.
#[must_use]
pub fn path_within(base: &Path, relative: &str) -> Option<PathBuf> {
    match resolve_path(base, relative) {
        PathResolution::Resolved(path) => Some(path),
        PathResolution::Escapes | PathResolution::Missing => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn canonical_bytes_are_two_space_indented_with_a_trailing_newline() {
        let bytes = to_canonical_bytes("t", &json!({"b": 1, "a": [1, 2]})).expect("serializes");
        assert_eq!(String::from_utf8(bytes).expect("utf8"), "{\n  \"a\": [\n    1,\n    2\n  ],\n  \"b\": 1\n}\n");
    }

    #[test]
    fn map_keys_are_sorted_but_an_integral_number_stays_integral() {
        let bytes = to_canonical_bytes("t", &json!({"z": 1, "a": 2})).expect("serializes");
        let text = String::from_utf8(bytes).expect("utf8");
        assert!(text.find("\"a\"").unwrap_or(usize::MAX) < text.find("\"z\"").unwrap_or(0), "{text}");
    }

    #[test]
    fn an_f64_is_written_with_its_decimal_point_and_an_integer_without() {
        // §5.7: "`1`, not `1.0`, where the type is integral; `1.5` where it is not". A `f64`
        // holding 612 writes `612.0`, which is the shortest form that round-trips *as an f64*.
        assert_eq!(to_canonical_bytes("t", &json!({"i": 1u32})).expect("ok"), b"{\n  \"i\": 1\n}\n".to_vec());
        assert_eq!(to_canonical_bytes("t", &json!({"f": 1.5f64})).expect("ok"), b"{\n  \"f\": 1.5\n}\n".to_vec());
        assert_eq!(to_canonical_bytes("t", &json!({"f": 612.0f64})).expect("ok"), b"{\n  \"f\": 612.0\n}\n".to_vec());
    }

    #[test]
    fn sha256_is_lowercase_hex() {
        assert_eq!(sha256_hex(b""), "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855");
        assert_eq!(sha256_hex(b"abc"), "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
    }

    #[test]
    fn an_escaping_path_is_never_resolved() {
        let root = crate::scratch::scratch_dir("path-within");
        let dir = root.join("bundle");
        let inside = dir.join("assets");
        fs::create_dir_all(&inside).expect("scratch dir");
        fs::write(inside.join("a.png"), b"x").expect("scratch file");
        fs::write(dir.join("outside.png"), b"x").expect("scratch file");
        fs::write(root.join("sibling.png"), b"x").expect("scratch file");

        let real = |p: &Path| p.canonicalize().expect("the scratch file exists");
        assert_eq!(resolve_path(&dir, "assets/a.png"), PathResolution::Resolved(real(&inside.join("a.png"))), "a path inside resolves");
        assert_eq!(
            resolve_path(&dir, "assets/../outside.png"),
            PathResolution::Resolved(real(&dir.join("outside.png"))),
            "`..` that stays inside is legal"
        );
        assert_eq!(resolve_path(&dir, "../sibling.png"), PathResolution::Escapes, "leaving the base is refused");
        assert_eq!(resolve_path(&dir, "../../etc/passwd"), PathResolution::Escapes, "escaping is refused even when the target is absent");
        assert_eq!(resolve_path(&dir, "/etc/passwd"), PathResolution::Escapes, "an absolute path is refused");
        assert_eq!(resolve_path(&dir, ""), PathResolution::Escapes, "an empty path names nothing");
        assert_eq!(resolve_path(&dir, "assets/missing.png"), PathResolution::Missing, "an absent file is Missing, not Escapes");
        assert!(path_within(&dir, "assets/a.png").is_some());
        assert!(path_within(&dir, "assets/missing.png").is_none());
        fs::remove_dir_all(&root).ok();
    }

    #[cfg(unix)]
    #[test]
    fn a_symlink_out_of_the_base_is_refused() {
        let root = crate::scratch::scratch_dir("path-symlink");
        let dir = root.join("bundle");
        let assets = dir.join("assets");
        fs::create_dir_all(&assets).expect("scratch dir");
        fs::write(root.join("outside.png"), b"x").expect("scratch file");
        std::os::unix::fs::symlink(root.join("outside.png"), assets.join("link.png")).expect("symlink");
        assert_eq!(resolve_path(&dir, "assets/link.png"), PathResolution::Escapes, "canonicalising before the check defeats a symlink");
        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn the_tree_excludes_the_manifest_exports_and_the_staging_directory() {
        let dir = crate::scratch::scratch_dir("scan-tree");
        fs::create_dir_all(dir.join("exports")).expect("temp dir");
        fs::create_dir_all(dir.join(".save-tmp")).expect("temp dir");
        fs::create_dir_all(dir.join("pages")).expect("temp dir");
        fs::write(dir.join("manifest.json"), b"{}").expect("temp file");
        fs::write(dir.join("exports/page.png"), b"x").expect("temp file");
        fs::write(dir.join(".save-tmp/project.json"), b"x").expect("temp file");
        fs::write(dir.join("pages/page-7.json"), b"{}\n").expect("temp file");
        let tree = scan_tree(&dir).expect("scans");
        assert_eq!(tree.keys().collect::<Vec<_>>(), vec!["pages/page-7.json"]);
        assert_eq!(tree["pages/page-7.json"].bytes, 3);
        fs::remove_dir_all(&dir).ok();
    }
}
