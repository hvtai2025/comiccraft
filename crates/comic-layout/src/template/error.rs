//! Why a template cannot be resolved, or laid into a page.
//!
//! One type, because every variant is the same kind of failure — "this template cannot produce
//! panel rectangles here" — and a caller that only reports it never has to tell them apart. The
//! messages are written for a creator (design §12.1: an actionable message, not an assertion).

/// A template that cannot be resolved, or cannot be laid into the page it was given.
///
/// The layout settings are checked here too: they are the template's own inputs (margin and
/// gutter), so a `NaN` margin is the template's refusal, not the plan's.
#[derive(Clone, Debug, PartialEq, thiserror::Error)]
pub enum TemplateError {
    /// A name no template answers to.
    ///
    /// The known names are spelled out in the message rather than collected from a list at
    /// runtime, so the message cannot drift from the set by being empty.
    #[error("`{name}` is not a template; the known templates are `two-tier`, `three-tier`, `four-tier`, `wide-top` and `grid-<rows>x<columns>`")]
    UnknownTemplate {
        /// The name as the caller wrote it, before normalization.
        name: String,
    },

    /// A `grid-<rows>x<columns>` name whose sides are zero, or past the bound this crate accepts.
    ///
    /// Bounded because a template name is **untrusted input**: it arrives in
    /// `ProjectSettings.default_template` (schema §5.3), a free-form string that a hand-edited or
    /// foreign bundle can set to anything, and a grid of `4294967295x4294967295` would otherwise
    /// ask for an allocation. That is a denial of service, not a layout.
    #[error("a grid template needs between 1 and {max} rows and columns, not {rows}x{columns}")]
    UnusableGrid {
        /// The rows the name asked for.
        rows: u32,
        /// The columns the name asked for.
        columns: u32,
        /// The largest side this crate lays out.
        max: u32,
    },

    /// A margin or gutter that is not a usable length.
    #[error("`{field}` must be a finite number of points at or above zero, not {value}")]
    InvalidSetting {
        /// Which setting failed: `margin` or `gutter`.
        field: &'static str,
        /// The value it was given.
        value: f64,
    },

    /// The margin leaves no area inside the page to lay panels into.
    #[error("a margin of {margin}pt leaves no content area inside a {width}x{height}pt page")]
    NoContentArea {
        /// The margin the settings asked for.
        margin: f64,
        /// The page's width, in points.
        width: f64,
        /// The page's height, in points.
        height: f64,
    },

    /// The gutters leave no room for a row's panels across the content width.
    #[error("{panels} panels across a {available}pt content width leave no room once a {gutter}pt gutter separates them")]
    NoRoomAcross {
        /// How many panels the row holds.
        panels: u32,
        /// The content width available, in points.
        available: f64,
        /// The gutter the settings asked for.
        gutter: f64,
    },

    /// The gutters leave no room for the shape's rows down the content height.
    #[error("{rows} rows in a {available}pt content height leave no room once a {gutter}pt gutter separates them")]
    NoRoomDown {
        /// How many rows the shape has.
        rows: u32,
        /// The content height available, in points.
        available: f64,
        /// The gutter the settings asked for.
        gutter: f64,
    },

    /// A panel rectangle could not be built from the page, margin and gutters.
    ///
    /// Reached only for a rectangle the arithmetic cannot represent — a sub-ULP size at an extreme
    /// coordinate — because the margin, the gutter and the content area are all checked before the
    /// rectangles are built. Kept rather than `expect`ed, so a page at the edge of `f64` is a
    /// report instead of a crash.
    #[error("a panel rectangle could not be built: {0}")]
    Rect(#[from] crate::PanelRectError),
}
