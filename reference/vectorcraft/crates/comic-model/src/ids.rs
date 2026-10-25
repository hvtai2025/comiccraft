//! Stable comic ids and the allocation counter.
//!
//! Schema §9. Every comic id is a `#[serde(transparent)]` newtype over `u64`, so a JSON id is a
//! bare number. Ids are **opaque** — never derive meaning from magnitude or order — and **never
//! reused**. They are allocated from `manifest.next_comic_id`, which only ever increases and is
//! repaired upward on load if a hand edit left it behind the ids the bundle already contains.
//!
//! The two visual identities are **not** here: `NodeId(u64)` and `Artboard.id: u32` come from
//! `vectorcraft-doc` (§9, design §5.3). This crate never mints a visual-object id.

use serde::{Deserialize, Serialize};

/// A stable comic entity id: the project, a registry, or a bundle.
///
/// [`AssetRegistryId`] is an alias of this, because §5.5 gives the registry a `ComicId` and the
/// id exists only so a future project can hold more than one registry without a schema change.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ComicId(pub u64);

impl std::fmt::Display for ComicId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

/// The `assets/index.json` registry's id (§5.5): a `ComicId` allocated once when the project is
/// created and written to `ComicProject.asset_registry_id`.
pub type AssetRegistryId = ComicId;

macro_rules! entity_ids {
    ($($(#[$meta:meta])* $name:ident),+ $(,)?) => {
        $(
            $(#[$meta])*
            #[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
            #[serde(transparent)]
            pub struct $name(pub u64);

            impl std::fmt::Display for $name {
                fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                    self.0.fmt(f)
                }
            }

            impl IdNewtype for $name {
                fn from_raw(raw: u64) -> Self {
                    Self(raw)
                }
                fn raw(self) -> u64 {
                    self.0
                }
            }
        )+
    };
}

entity_ids! {
    /// A chapter (§5.4).
    ChapterId,
    /// A scene (§5.4).
    SceneId,
    /// A character (§5.4).
    CharacterId,
    /// A location (§5.4).
    LocationId,
    /// A style profile (§5.4).
    StyleProfileId,
    /// A page (§6.1). The page's file is keyed by this id, never by an ordinal (§3).
    PageId,
    /// A panel (§6.2).
    PanelId,
    /// A dialogue entry (§6.3).
    DialogueId,
    /// A lettering object — balloon, caption or sound effect (§6.4).
    LetteringId,
    /// An asset in the registry (§7).
    AssetId,
    /// A generation job (§8.1).
    JobId,
    /// A review decision (§8.2).
    ReviewDecisionId,
}

impl IdNewtype for ComicId {
    fn from_raw(raw: u64) -> Self {
        Self(raw)
    }
    fn raw(self) -> u64 {
        self.0
    }
}

/// What [`IdAllocator`] and [`ComicBundle::allocate`](crate::ComicBundle::allocate) need of an id
/// type: a raw round trip, so one counter can issue every kind.
///
/// This exists so allocation has **one** implementation. A second one per id type is the place a
/// reuse bug hides.
pub trait IdNewtype: Copy {
    /// Wrap a raw counter value.
    fn from_raw(raw: u64) -> Self;
    /// The raw value, for the counter and for comparisons across kinds (§13 rule 2).
    fn raw(self) -> u64;
}

/// The persisted comic id counter (§9).
///
/// Held by [`Manifest::next_comic_id`](crate::Manifest::next_comic_id) — one authority for the
/// counter, so an in-memory copy cannot drift from the persisted one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct IdAllocator {
    next: u64,
}

impl IdAllocator {
    /// A counter that will issue `next` and then increase.
    #[must_use]
    pub fn new(next: u64) -> Self {
        Self { next }
    }

    /// The value the next allocation will use.
    #[must_use]
    pub fn next_raw(self) -> u64 {
        self.next
    }

    /// Issue an id, or refuse when the counter has reached `u64::MAX`.
    ///
    /// Refusing at the end of the range rather than wrapping is what keeps §9's "never reuse an
    /// id" true at the boundary; `u64::MAX` is left unissued so that `next` can always be
    /// strictly greater than every id present.
    pub fn allocate<K: IdNewtype>(&mut self) -> Result<K, crate::Error> {
        if self.next == u64::MAX {
            return Err(crate::Error::IdsExhausted { next: self.next });
        }
        let raw = self.next;
        self.next += 1;
        Ok(K::from_raw(raw))
    }

    /// Repair the counter upward so it is greater than every id present (§9, §14 step 4).
    ///
    /// Returns the new value when it changed, so the caller can report the repair. It never
    /// lowers the counter: an id that was issued and then removed must stay unusable.
    pub fn repair_upward(&mut self, highest_present: u64) -> Option<u64> {
        if self.next > highest_present {
            return None;
        }
        self.next = highest_present.saturating_add(1);
        Some(self.next)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_kind_is_a_bare_number_and_round_trips() {
        assert_eq!(serde_json::to_string(&PageId(7)).expect("serializes"), "7");
        assert_eq!(serde_json::from_str::<PageId>("7").expect("deserializes"), PageId(7));
        assert_eq!(ComicId(7).to_string(), "7");
        assert_eq!(ComicId(7), AssetRegistryId::from_raw(7), "the registry id is a ComicId");
    }

    #[test]
    fn allocation_issues_each_value_once() {
        let mut ids = IdAllocator::new(4);
        assert_eq!(ids.allocate::<PageId>().expect("room"), PageId(4));
        assert_eq!(ids.allocate::<PanelId>().expect("room"), PanelId(5));
        assert_eq!(ids.next_raw(), 6);
    }

    #[test]
    fn allocation_refuses_the_end_of_the_range_rather_than_reusing_max() {
        let mut ids = IdAllocator::new(u64::MAX - 1);
        assert_eq!(ids.allocate::<JobId>().expect("room"), JobId(u64::MAX - 1));
        assert!(ids.allocate::<JobId>().is_err(), "u64::MAX must stay unissued");
    }

    #[test]
    fn repair_only_ever_moves_the_counter_up() {
        let mut ids = IdAllocator::new(3);
        assert_eq!(ids.repair_upward(9), Some(10), "behind the ids present: repaired");
        assert_eq!(ids.repair_upward(4), None, "already greater: untouched");
        assert_eq!(ids.next_raw(), 10);
    }

    #[test]
    fn repair_at_the_top_of_the_range_saturates_instead_of_wrapping() {
        let mut ids = IdAllocator::new(1);
        assert_eq!(ids.repair_upward(u64::MAX), Some(u64::MAX));
        assert!(ids.allocate::<PageId>().is_err(), "a saturated counter issues nothing");
    }
}
