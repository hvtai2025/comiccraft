//! The visual document as the comic model needs to see it.
//!
//! `crates/format` sits at L3 and this crate at L2, so this crate may **not** depend on it and
//! never parses a `.vectorcraft` file. That is not a shortcut: the two versions move
//! independently (§2), and `crates/format` behaviour is not changed by ComicCraft (§11). What
//! rules 4, 7 and 15 need is not the document itself but three facts about it, which the caller
//! (the command layer, which already holds the loaded `Document`) supplies:
//!
//! - which artboards exist, and their `rect`s (rules 4 and 15),
//! - which node ids exist (rule 7),
//! - the document's bytes, for the ordered save of §10 and for `manifest.files`.
//!
//! [`DocumentFacts::from_document`] builds the first two from a `vectorcraft_doc::Document`, so
//! the caller does not hand-roll the walk.
//!
//! **The artboard-id hazard.** `Artboard.id` is allocated as `max(id) + 1` and there is no
//! `fix_next_id()` equivalent for artboards (audit §13 U1), so a deleted artboard's number can come
//! back. Four guards stand until that upstream change lands — comic commands own artboard
//! lifecycle, [`Manifest::retired_artboard_ids`](crate::Manifest::retired_artboard_ids) remembers
//! what was deleted, rule 4 cross-checks, and
//! [`ComicBundle::assignable_artboards`](crate::ComicBundle::assignable_artboards) keeps a page
//! from being given a retired number (design §5.3.1).

use std::collections::BTreeSet;

use vectorcraft_doc::{Document, NodeId};
use vectorcraft_geom::Rect;

/// One artboard of the visual document, as a page's visual home (§5.1.1).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ArtboardFacts {
    /// `Artboard.id` — the id a `Page.artboard_id` names.
    pub id: u32,
    /// The artboard's rectangle in document points. **This is the page's size** (§6.1); there is
    /// no second copy to compare against, which is why rule 15 only warns.
    pub rect: Rect,
}

/// What rules 4, 7 and 15 need to know about the visual document.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DocumentFacts {
    /// Every artboard in the document, by id.
    pub artboards: Vec<ArtboardFacts>,
    /// Every node id in the document, for rule 7.
    pub nodes: BTreeSet<NodeId>,
}

impl DocumentFacts {
    /// Read the facts out of a loaded document.
    #[must_use]
    pub fn from_document(doc: &Document) -> Self {
        let mut nodes = BTreeSet::new();
        doc.walk(|n| {
            nodes.insert(n.id);
        });
        Self { artboards: doc.artboards.iter().map(|a| ArtboardFacts { id: a.id, rect: a.rect }).collect(), nodes }
    }

    /// Whether `id` names an artboard (rule 4).
    #[must_use]
    pub fn has_artboard(&self, id: u32) -> bool {
        self.artboards.iter().any(|a| a.id == id)
    }

    /// The artboard `id` names (rule 15).
    #[must_use]
    pub fn artboard(&self, id: u32) -> Option<ArtboardFacts> {
        self.artboards.iter().copied().find(|a| a.id == id)
    }
}

/// The visual document's bytes, on their way into a bundle.
///
/// The caller serializes the document through `crates/format` **and verifies it by loading it
/// back** before handing it over — §10 step 3, which this crate cannot do for itself.
#[derive(Clone, Debug, PartialEq)]
pub struct DocumentPayload {
    /// The document's path from the bundle root, `/`-separated. Normally
    /// `manifest.document.relative_path`.
    pub relative_path: String,
    /// The `format::File.version` the caller wrote. Becomes
    /// `VisualDocumentRef.format_version`, which is the only place the comic schema records it
    /// (§2: the two versions move independently).
    pub format_version: u32,
    /// The file's bytes.
    pub bytes: Vec<u8>,
}
