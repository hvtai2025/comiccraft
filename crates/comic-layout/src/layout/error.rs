//! Why a page could not be planned.

use crate::PanelRectError;
use crate::template::error::TemplateError;

/// A page that could not be planned.
///
/// Two things can go wrong, and the split is deliberate: the **template** cannot be resolved or
/// cannot be laid into this page ([`TemplateError`], transparent so its message reaches the caller
/// unchanged), or the **request** does not describe a page that can exist — a panel count the
/// template does not place, or a page rectangle a layout cannot be built in.
#[derive(Clone, Debug, PartialEq, thiserror::Error)]
pub enum LayoutError {
    /// The template could not be resolved, or laid into the page.
    #[error(transparent)]
    Template(#[from] TemplateError),

    /// The requested panel count is not the template's.
    ///
    /// Reported rather than padded or truncated: a template that placed a different number of panels
    /// than the caller asked for would be laying out a page nobody requested, and the caller's panel
    /// records would not match the rectangles.
    #[error("template `{template}` places {template_count} panels, not the {requested} requested")]
    PanelCountMismatch {
        /// The template's name, as `ProjectSettings.default_template` spells it.
        template: String,
        /// How many panels the caller asked for.
        requested: u32,
        /// How many the template places.
        template_count: u32,
    },

    /// The page rectangle a layout would be built in is not a usable rectangle.
    ///
    /// Reached for a `NaN`, infinite, inverted or zero-area artboard. It is reported before any
    /// rectangle is computed, so a malformed page never produces panel rectangles at all.
    #[error("the page rectangle cannot be laid out in: {0}")]
    Artboard(#[from] PanelRectError),
}
