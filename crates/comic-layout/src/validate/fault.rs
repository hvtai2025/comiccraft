//! The geometric checks of design §12.1, as structured faults.
//!
//! A fault is structured rather than phrased so a caller — and this module's tests — can act on one
//! without matching text; [`Fault::describe`] turns it into the [`Finding`] a command returns. The
//! check itself is [`crate::validate::faults`], which measures rectangles and nothing else.

use vectorcraft_comic_model::{Finding, Severity};

/// One geometric problem with a set of panel rectangles.
///
/// Every index refers to the position of a panel **in the set that was checked**, because that is all
/// the check knows: the caller supplies the label, since only the caller knows whether the set is a
/// plan's `Page.panels[3]` or a page's `Panel(9)`.
///
/// Severity is split by "would anything break?" rather than by "is this suspicious?":
///
/// | Fault | Severity | Why |
/// |---|---|---|
/// | [`Fault::Overlap`] | error | two panels sharing area is a broken page |
/// | [`Fault::TooSmall`] | error | a panel below the minimum cannot carry artwork or a balloon |
/// | [`Fault::OutsidePage`] | error | content off the page is lost in print |
/// | [`Fault::GutterTooThin`] | warning | a tight pair still renders; a creator may want it |
/// | [`Fault::InSafeMargin`] | warning | a splash panel deliberately bleeding into the margin |
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Fault {
    /// Two panels share area.
    Overlap {
        /// The first panel's index.
        first: usize,
        /// The second panel's index.
        second: usize,
    },

    /// Two neighbouring panels are closer than the minimum gutter.
    GutterTooThin {
        /// The first panel's index.
        first: usize,
        /// The second panel's index.
        second: usize,
        /// The gap between them, in points. Zero when they share an edge.
        gap: f64,
        /// The minimum the check was given, in points.
        minimum: f64,
    },

    /// A panel is smaller than the minimum on at least one axis.
    TooSmall {
        /// The panel's index.
        panel: usize,
        /// Its width, in points.
        width: f64,
        /// Its height, in points.
        height: f64,
        /// The minimum width the check was given, in points.
        min_width: f64,
        /// The minimum height the check was given, in points.
        min_height: f64,
    },

    /// A panel is not inside the page.
    OutsidePage {
        /// The panel's index.
        panel: usize,
    },

    /// A panel is inside the page but crosses a safe margin.
    InSafeMargin {
        /// The panel's index.
        panel: usize,
    },
}

impl Fault {
    /// How seriously to take this fault.
    #[must_use]
    pub fn severity(&self) -> Severity {
        match self {
            Self::Overlap { .. } | Self::TooSmall { .. } | Self::OutsidePage { .. } => Severity::Error,
            Self::GutterTooThin { .. } | Self::InSafeMargin { .. } => Severity::Warning,
        }
    }

    /// Phrase the fault for a creator.
    ///
    /// `label` names the panel at a given index in the checked set — `Page.panels[3]` for a plan,
    /// `Panel(9)` for a page's records. The finding's `rule` is `None`: these are design §12.1's
    /// layout checks, not one of schema §13's eighteen referential rules.
    #[must_use]
    pub fn describe(&self, label: &dyn Fn(usize) -> String) -> Finding {
        let (subject, message) = match self {
            Self::Overlap { first, second } => (label(*first), format!("overlaps {}: two panels cannot share area", label(*second))),
            Self::GutterTooThin { first, second, gap, minimum } => {
                (label(*first), format!("sits {gap}pt from {}, closer than the {minimum}pt minimum gutter", label(*second)))
            }
            Self::TooSmall { panel, width, height, min_width, min_height } => {
                (label(*panel), format!("is {width}x{height}pt, below the {min_width}x{min_height}pt minimum panel size"))
            }
            Self::OutsidePage { panel } => (label(*panel), "is not inside the page".to_string()),
            Self::InSafeMargin { panel } => {
                (label(*panel), "crosses the safe margin; a panel that deliberately bleeds should be confirmed".to_string())
            }
        };
        match self.severity() {
            Severity::Error => Finding::error(None, subject, message),
            Severity::Warning => Finding::warning(None, subject, message),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_fault_is_phrased_for_a_creator_with_the_panels_it_names() {
        // Every variant, because this is the only place the phrasing is pinned: a message that lost
        // the panel it is about, or the two panels of a pair, would leave a creator guessing.
        let label = |index: usize| format!("Panel({index})");

        let overlap = Fault::Overlap { first: 0, second: 1 }.describe(&label);
        assert_eq!(overlap.subject, "Panel(0)");
        assert_eq!(overlap.message, "overlaps Panel(1): two panels cannot share area");
        assert_eq!(overlap.severity, Severity::Error);
        assert_eq!(overlap.rule, None, "a layout check is not one of the eighteen referential rules");

        let thin = Fault::GutterTooThin { first: 0, second: 1, gap: 0.0, minimum: 6.0 }.describe(&label);
        assert_eq!(thin.subject, "Panel(0)");
        assert_eq!(thin.message, "sits 0pt from Panel(1), closer than the 6pt minimum gutter");
        assert_eq!(thin.severity, Severity::Warning);

        let small = Fault::TooSmall { panel: 2, width: 10.0, height: 20.0, min_width: 48.0, min_height: 48.0 }.describe(&label);
        assert_eq!(small.subject, "Panel(2)");
        assert_eq!(small.message, "is 10x20pt, below the 48x48pt minimum panel size");
        assert_eq!(small.severity, Severity::Error);

        let outside = Fault::OutsidePage { panel: 3 }.describe(&label);
        assert_eq!(outside.subject, "Panel(3)");
        assert_eq!(outside.message, "is not inside the page");
        assert_eq!(outside.severity, Severity::Error);

        let margin = Fault::InSafeMargin { panel: 4 }.describe(&label);
        assert_eq!(margin.subject, "Panel(4)");
        assert!(margin.message.contains("safe margin"), "{}", margin.message);
        assert_eq!(margin.severity, Severity::Warning);
        // The split a caller acts on: a plan must not be applied over an error, and may be applied
        // over a warning.
        assert!(!margin.is_error());
        assert!(small.is_error());
    }
}
