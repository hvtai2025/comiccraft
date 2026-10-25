//! `comic.lettering.*` and `comic.dialogue.update`: balloons, captions, sound effects and the words
//! they carry.
//!
//! `comic-layout` **computes a plan and never takes `&mut Document`** (design §8). This module is
//! the other half: it hands the plan's geometry to `vectorcraft-doc` as ordinary editable objects —
//! a frame path, a tail path and an area text object — and writes the `Lettering` record whose ids
//! point at them. A `LetteringPlan` holds no `NodeId`, because the layout crate never mints one
//! (design §9); the ids come from here.
//!
//! # Who owns the words
//!
//! Schema rule 8 is structural: a **spoken** kind (`speech`, `thought`, `shout`, `whisper`) takes
//! its words from the linked `Dialogue` and must have `text: null`; a `narration` or `sound_effect`
//! carries its own `text` and must have `dialogue_id: null`. `comic-layout`'s `TextSource` refuses
//! a request that says otherwise, so the two can never both claim the words (design §5.2.1).
//!
//! # `set_text` against `dialogue.update`
//!
//! Both write the words, and both put them in the text object so the page shows them. They differ
//! in one thing, and it is the thing M1's acceptance test 5 pins: **`comic.dialogue.update` touches
//! no typography and no geometry.** It changes the record and the words on the page and leaves the
//! balloon's frame, tail, `TextLayoutSettings` and size exactly as they were. `comic.lettering.set_text`
//! is the *lettering* command and re-fits the box under the record's `overflow_policy`, which may
//! move the frame or the tail.

use serde_json::{Value, json};
use vectorcraft_color::{Color, Paint};
use vectorcraft_comic_layout::fit::{FitTextStyle, layout_settings};
use vectorcraft_comic_layout::lettering::{LetteringPlan, LetteringRequest, LetteringStyle, TextSource, new_balloon, refit_balloon, style_of};
use vectorcraft_comic_model::{
    ApprovalStatus, CharacterId, Dialogue, DialogueId, Lettering, LetteringKind, OverflowPolicy, PageId, PanelId, TextAlign, TextLayoutSettings,
};
use vectorcraft_doc::{Appearance, AppearanceItem, AreaFit, CharStyle, Dash, Document, Node, NodeId, NodeKind, ParaStyle, TextKind, TextObject};
use vectorcraft_geom::{PathData, Point, Rect, shapes};
use vectorcraft_text::FontDb;

use super::*;
use crate::{EngineError, Result, Session};

pub fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!(
            "comic.lettering.add",
            "Add Lettering",
            ["Window", "Comic"],
            None,
            "{page: PageId, panel: PanelId, kind: \"speech\"|\"thought\"|\"narration\"|\"shout\"|\"whisper\"|\"sound_effect\", text?: the words (for a spoken kind a new Dialogue record is made when `dialogue` is absent; required for narration and sound_effect), dialogue?: DialogueId (a spoken kind already saying something), at?: [x, y] the box's anchor (default: the panel frame's centre), tail?: [x, y] what a spoken kind points at (default: the frame's bottom centre), overflowPolicy?: \"warn\" (default)|\"shrink\"|\"resize_frame\", style?: {font?, size?, leading?, tracking?, align?, padding?} (the face defaults to the kind's ADR-017 face, or to the family the project names in `default_lettering_style` — design §8.3, schema §5.3)} plan the balloon or caption with comic-layout and write it as editable objects: the frame path (Lettering.frame_node_id), the tail path (tail_node_id) and an area text object (text_node_id), plus the Lettering record → {lettering, dialogue?, kind, frame, tail?, text_node, bounds, text_layout, findings: [one report per face the plan asked for that is not available, naming the family, the style, the status and the face that stands in — the CRAFT_FONTS-empty contract]}. A lock does not apply here (§6.2): this adds a balloon of its own and moves no panel object. A `Dialogue` has **one owner**, the panel `Dialogue.panel_id` names (design §6.1): a balloon given another panel's `dialogue` shows that record's words without claiming it, so only the owner lists it in `Panel.dialogue_ids`. When `panel_id` names a panel that no longer exists — a dialogue `comic.panel.delete {reuse}` detached (§11.1) — **this panel adopts it**, which is the move the §11.1 row promises and the way back from the rule-1 Error. One undo step",
            has_comic,
            add
        ),
        cmd!(
            "comic.lettering.set_text",
            "Set Lettering Text",
            ["Window", "Comic"],
            None,
            "{lettering: LetteringId, text: the new words} write the words where they belong — the linked Dialogue for a spoken kind, the record itself for narration and sound_effect (rule 8) — put them in the text object, and re-fit the box under the record's overflow_policy, which may move the frame or the tail → {lettering, kind, text, text_layout, overflow_policy, bounds, outcome, moved, findings}. comic.dialogue.update is the one that leaves the typography and geometry alone. A spoken kind's record is resolved **within the balloon's own page**: a balloon that shows a dialogue owned by a panel on another page (legal — no rule ties a `Lettering` to its dialogue's page, §6.1) is refused here rather than reached across files, so use comic.dialogue.update, which finds the record by id. One undo step",
            has_comic,
            set_text
        ),
        cmd!(
            "comic.lettering.set_style",
            "Set Lettering Style",
            ["Window", "Comic"],
            None,
            "{lettering: LetteringId, font?, size?, leading?: pt (null clears Auto), tracking?, align?: \"left\"|\"center\"|\"right\", padding?, fitToFrame?: bool, overflowPolicy?: \"warn\"|\"shrink\"|\"resize_frame\"} set Lettering.text_layout_settings (and overflow_policy), put the same typography on the text object, and re-fit the box → {lettering, text_layout, overflow_policy, bounds, outcome, moved, findings}. One undo step",
            has_comic,
            set_style
        ),
        cmd!(
            "comic.dialogue.update",
            "Update Dialogue",
            ["Window", "Comic"],
            None,
            "{dialogue: DialogueId, text?, speaker?: CharacterId (null for narration), language?: BCP-47 (null clears it), readingOrderIndex?: u16, approval?: \"pending\"|\"approved\"|\"rejected\"} change the Dialogue record, and the words it owns where the page shows them. **Touches no typography and no geometry**: no frame, tail, text-layout setting or size changes (M1 acceptance test 5) → {dialogue, panel, text, speaker, language, readingOrderIndex, approval, typographyTouched: false, geometryTouched: false}. One undo step",
            has_comic,
            dialogue_update
        ),
    ]
}

// ---------- parameters ----------

fn kind_param(p: &Value, cmd: &str) -> Result<LetteringKind> {
    match str_param(p, "kind") {
        Some("speech") => Ok(LetteringKind::Speech),
        Some("thought") => Ok(LetteringKind::Thought),
        Some("narration") => Ok(LetteringKind::Narration),
        Some("shout") => Ok(LetteringKind::Shout),
        Some("whisper") => Ok(LetteringKind::Whisper),
        Some("sound_effect") => Ok(LetteringKind::SoundEffect),
        other => Err(bad(cmd, format!("`kind` is speech, thought, narration, shout, whisper or sound_effect, not {other:?}"))),
    }
}

fn kind_name(kind: LetteringKind) -> &'static str {
    match kind {
        LetteringKind::Speech => "speech",
        LetteringKind::Thought => "thought",
        LetteringKind::Narration => "narration",
        LetteringKind::Shout => "shout",
        LetteringKind::Whisper => "whisper",
        LetteringKind::SoundEffect => "sound_effect",
    }
}

fn overflow_name(policy: OverflowPolicy) -> &'static str {
    match policy {
        OverflowPolicy::Warn => "warn",
        OverflowPolicy::Shrink => "shrink",
        OverflowPolicy::ResizeFrame => "resize_frame",
    }
}

fn overflow_param(p: &Value, cmd: &str) -> Result<OverflowPolicy> {
    match str_param(p, "overflowPolicy") {
        None | Some("warn") => Ok(OverflowPolicy::Warn),
        Some("shrink") => Ok(OverflowPolicy::Shrink),
        Some("resize_frame") => Ok(OverflowPolicy::ResizeFrame),
        Some(other) => Err(bad(cmd, format!("`overflowPolicy` is warn, shrink or resize_frame, not {other:?}"))),
    }
}

fn align_param(p: &Value, cmd: &str) -> Result<TextAlign> {
    match str_param(p, "align") {
        Some("left") => Ok(TextAlign::Left),
        Some("center") => Ok(TextAlign::Center),
        Some("right") => Ok(TextAlign::Right),
        other => Err(bad(cmd, format!("`align` is left, center or right, not {other:?}"))),
    }
}

fn point_param(p: &Value, key: &str, cmd: &str) -> Result<Option<Point>> {
    match p.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Array(a)) if a.len() == 2 => match (a[0].as_f64(), a[1].as_f64()) {
            (Some(x), Some(y)) => Ok(Some(Point::new(x, y))),
            _ => Err(bad(cmd, format!("`{key}` must be [x, y] numbers"))),
        },
        Some(_) => Err(bad(cmd, format!("`{key}` must be [x, y]"))),
    }
}

/// A finite, positive length parameter of a nested object (the `style` object of
/// `comic.lettering.add`).
fn positive(o: &serde_json::Map<String, Value>, key: &str, cmd: &str) -> Result<f64> {
    let v = o.get(key).and_then(Value::as_f64).ok_or_else(|| bad(cmd, format!("`style.{key}` must be a number")))?;
    if !(v.is_finite() && v > 0.0) {
        return Err(bad(cmd, format!("`style.{key}` must be a finite, positive number of points, not {v}")));
    }
    Ok(v)
}

// ---------- the style a record carries ----------

/// The family the project asks its lettering to be set in, if it names one (schema §5.3,
/// design §8.3).
///
/// `ProjectSettings.default_lettering_style` is a **name**, and this is the reader that makes it
/// mean something: before M2-L2 it was written by nothing and read by nothing, so a project's chosen
/// face never reached a plan and §8.3's "the lettering then falls back with a missing-font report"
/// was only ever about the *kind's* face. The name is read as the **family**; the style within it
/// (Regular/Bold/Italic) stays the kind's, because a single string cannot carry a pair — a project
/// that wants a different weight per kind says so on the record with `comic.lettering.set_style`.
/// A blank or whitespace-only name is no name: `comic-layout` skips an empty family as "not a font".
fn project_face(b: &ComicBundle) -> Option<String> {
    let name = b.project.settings.default_lettering_style.as_ref()?.trim();
    (!name.is_empty()).then(|| name.to_string())
}

/// The kind's default style, with the project's chosen face in place of the kind's default family.
fn style_for_project(b: &ComicBundle, kind: LetteringKind) -> LetteringStyle {
    let mut style = style_of(kind);
    if let Some(face) = project_face(b) {
        style.text.family = face;
    }
    style
}

/// The typography a `Lettering` record currently asks for, as a plan style: the kind's default (with
/// the project's chosen face, when there is one) with every setting the record states applied on
/// top. A record that states a `font` keeps it — the project's setting is a **default**, and the
/// per-lettering `font` is what selects a face (schema §6.6).
fn style_from_record(b: &ComicBundle, l: &Lettering) -> LetteringStyle {
    let mut style = style_for_project(b, l.kind);
    let s = &l.text_layout_settings;
    if let Some(font) = &s.font {
        style.text.family = font.clone();
    }
    if let Some(size) = s.size.filter(|v| v.is_finite() && *v > 0.0) {
        style.text.size = size;
    }
    if let Some(leading) = s.leading.filter(|v| v.is_finite() && *v > 0.0) {
        style.text.leading = Some(leading);
    }
    if let Some(tracking) = s.tracking.filter(|v| v.is_finite()) {
        style.text.tracking = tracking;
    }
    if let Some(align) = s.align {
        style.text.align = align;
    }
    if let Some(padding) = s.padding.filter(|v| v.is_finite() && *v >= 0.0) {
        style.text.padding = padding;
    }
    style
}

/// Apply the optional `style` object of `comic.lettering.add` to a kind's default style.
fn style_with_overrides(mut style: LetteringStyle, p: &Value, cmd: &str) -> Result<LetteringStyle> {
    let Some(o) = p.get("style").filter(|v| !v.is_null()) else { return Ok(style) };
    let Value::Object(o) = o else { return Err(bad(cmd, "`style` must be an object")) };
    if let Some(font) = o.get("font").and_then(Value::as_str) {
        style.text.family = font.to_string();
    }
    if o.get("size").is_some_and(|v| !v.is_null()) {
        style.text.size = positive(o, "size", cmd)?;
    }
    if o.get("leading").is_some_and(|v| !v.is_null()) {
        style.text.leading = Some(positive(o, "leading", cmd)?);
    }
    if o.get("tracking").is_some_and(|v| !v.is_null()) {
        let v = o.get("tracking").and_then(Value::as_f64).ok_or_else(|| bad(cmd, "`style.tracking` must be a number"))?;
        if !v.is_finite() {
            return Err(bad(cmd, format!("`style.tracking` must be finite, not {v}")));
        }
        style.text.tracking = v;
    }
    if o.get("padding").is_some_and(|v| !v.is_null()) {
        let v = o.get("padding").and_then(Value::as_f64).ok_or_else(|| bad(cmd, "`style.padding` must be a number"))?;
        if !(v.is_finite() && v >= 0.0) {
            return Err(bad(cmd, format!("`style.padding` must be finite and not negative, not {v}")));
        }
        style.text.padding = v;
    }
    if o.get("align").is_some_and(|v| !v.is_null()) {
        style.text.align = align_param(&Value::Object(o.clone()), cmd)?;
    }
    Ok(style)
}

// ---------- objects ----------

/// The paint a balloon or caption outline is created with: a white fill and a black outline, dashed
/// for a whisper (a stroke property, not a second path).
fn outline_appearance(dashed: bool) -> Appearance {
    let mut a = Appearance::basic(Paint::solid(Color::WHITE), Paint::solid(Color::BLACK), 1.0);
    if dashed {
        for item in &mut a.items {
            if let AppearanceItem::Stroke(s) = item {
                s.dash = Some(Dash { pattern: vec![4.0, 3.0], offset: 0.0, align_corners: false });
            }
        }
    }
    a
}

/// The character style a plan's typography asks for, as the document spells it.
fn char_style(style: &FitTextStyle) -> CharStyle {
    CharStyle {
        font_family: style.family.clone(),
        font_style: style.style.clone(),
        size: style.size,
        leading: style.leading,
        tracking: style.tracking,
        fill: Paint::solid(Color::BLACK),
        ..CharStyle::default()
    }
}

/// An area text object at `frame`, in the plan's typography, holding `words`.
fn text_object(words: &str, frame: Rect, style: &FitTextStyle, fit: AreaFit) -> TextObject {
    let mut t = TextObject::point(frame.origin(), words, char_style(style));
    t.kind = TextKind::Area { frame: shapes::rectangle(Rect::new(0.0, 0.0, frame.width(), frame.height())) };
    t.para = ParaStyle { justify: vectorcraft_comic_layout::fit::justify(style.align), ..ParaStyle::default() };
    t.area.fit = fit;
    super::super::typecmd::refresh_bounds(&mut t);
    t
}

/// Replace a text object's content in place, keeping its placement. Used where the words change but
/// the typography and geometry must not (schema rule 8's one owner, and M1 acceptance test 5).
fn rewrite_text(d: &mut Document, id: NodeId, words: &str) -> Result<()> {
    let Some(node) = d.node_mut(id) else { return Err(EngineError::NoNode(id)) };
    let NodeKind::Text(t) = &mut node.kind else {
        return Err(EngineError::Other(format!("the lettering's text object {} is not a type object", id.0)));
    };
    let mut fresh = t.clone();
    match fresh.runs.first_mut() {
        Some(run) => run.text = words.to_string(),
        None => fresh.runs.push(vectorcraft_doc::TextRun::new(words, CharStyle::default())),
    }
    fresh.cached_bounds = None;
    fresh.cached_baselines.clear();
    super::super::typecmd::refresh_bounds(&mut fresh);
    let xf = t.xf;
    fresh.xf = xf;
    *t = fresh;
    Ok(())
}

fn rect_json(r: Rect) -> Value {
    json!([r.x0, r.y0, r.width(), r.height()])
}

fn layout_json(s: &TextLayoutSettings) -> Value {
    json!({
        "font": s.font,
        "size": s.size,
        "leading": s.leading,
        "tracking": s.tracking,
        "align": s.align.map(|a| match a { TextAlign::Left => "left", TextAlign::Center => "center", TextAlign::Right => "right" }),
        "padding": s.padding,
        "fitToFrame": s.fit_to_frame,
    })
}

fn outcome_name(outcome: vectorcraft_comic_layout::fit::FitOutcome) -> &'static str {
    use vectorcraft_comic_layout::fit::FitOutcome;
    match outcome {
        FitOutcome::Fits { .. } => "fits",
        FitOutcome::Warned => "warned",
        FitOutcome::Shrunk { .. } => "shrunk",
        FitOutcome::Resized { .. } => "resized",
        FitOutcome::Blocked { .. } => "blocked",
        FitOutcome::Unfittable => "unfittable",
    }
}

// ---------- comic.lettering.add ----------

fn add(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "comic.lettering.add";
    let page = PageId(super::panel::raw_id(p, "page", C)?);
    let panel = PanelId(super::panel::raw_id(p, "panel", C)?);
    let kind = kind_param(p, C)?;
    let overflow = overflow_param(p, C)?;
    // The project's chosen face, when it names one (schema §5.3, design §8.3), then the call's own
    // `style` overrides on top of it.
    let style = style_with_overrides(style_for_project(&open(s)?.bundle, kind), p, C)?;
    let (artboard_id, frame) = {
        let b = &open(s)?.bundle;
        let (page_of_panel, index) = locate_panel(b, panel)?;
        if page_of_panel != page {
            return Err(bad(C, format!("panel {} is on page {}, not page {}", panel.0, page_of_panel.0, page.0)));
        }
        let file = page_file(b, page)?;
        (file.page.artboard_id, file.panels[index].frame_node_id)
    };
    let panel_rect = s
        .doc()?
        .doc
        .node(frame)
        .and_then(|n| n.geometric_bounds())
        .ok_or_else(|| EngineError::Other(format!("the panel's frame {} has no geometry", frame.0)))?;
    let at = point_param(p, "at", C)?.unwrap_or_else(|| panel_rect.center());
    let tail = point_param(p, "tail", C)?.unwrap_or_else(|| Point::new(panel_rect.center().x, panel_rect.y1));

    // The words, and which record owns them (schema rule 8). Read before the edit so the plan is
    // built from a stable snapshot.
    let given_dialogue = p.get("dialogue").and_then(Value::as_u64).map(DialogueId);
    let given_text = str_param(p, "text").map(str::to_string);
    // A `Dialogue` has exactly one owner: the panel `Dialogue.panel_id` names (design §6.1 — one
    // authoritative copy, which is what a second owner would break). So this reads that owner, and
    // whether it still exists, before deciding anything.
    let given = match given_dialogue {
        Some(id) => {
            let b = &open(s)?.bundle;
            let (d_page, index) = locate_dialogue(b, id)?;
            let owner = page_file(b, d_page)?.dialogue[index].panel_id;
            Some((page_file(b, d_page)?.dialogue[index].text.clone(), owner, locate_panel(b, owner).is_ok()))
        }
        None => None,
    };
    let dialogue_text = given.as_ref().map(|(text, _, _)| text.clone());
    // Re-homing: the owner the dialogue names is gone (its panel was deleted and the dialogue was
    // detached rather than deleted — design §11.1's "the text is moved rather than retyped"), so
    // this balloon's panel adopts it and `Dialogue.panel_id` names a panel again.
    let adopt = given.as_ref().is_some_and(|(_, owner, owner_exists)| !owner_exists && *owner != panel);
    let owns = given.as_ref().is_none_or(|(_, owner, owner_exists)| !owner_exists || *owner == panel);
    let words: String = if kind.is_spoken() {
        match (dialogue_text, given_text.clone()) {
            (Some(text), _) => text,
            (None, Some(text)) => text,
            (None, None) => return Err(bad(C, "a spoken lettering must say something: give `text` or `dialogue`")),
        }
    } else {
        given_text.clone().ok_or_else(|| bad(C, format!("a {} carries its own words: give `text`", kind_name(kind))))?
    };
    let source = if kind.is_spoken() { TextSource::Dialogue(words.as_str()) } else { TextSource::Own(words.as_str()) };
    let request = LetteringRequest::new(kind, source).placed_at(at).with_policy(overflow).with_style(style.clone());
    let request = if kind.is_spoken() { request.with_tail(tail) } else { request };
    let plan = new_balloon(FontDb::global(), &request).map_err(|e| bad(C, format!("{e}")))?;

    let (lettering_id, dialogue_id) = comic_edit(s, "Add Lettering", |s| {
        let id = {
            let oc = open_mut(s)?;
            oc.bundle.allocate::<LetteringId>().map_err(|e| EngineError::Other(format!("{e}")))?
        };
        let dialogue_id = if kind.is_spoken() {
            match given_dialogue {
                Some(existing) => {
                    if adopt {
                        // The owner is gone, so this is the §11.1 move: the balloon's panel adopts
                        // the detached dialogue, which is what re-attaches `Dialogue.panel_id` to a
                        // panel that exists. The words are carried, never retyped.
                        //
                        // **The record keeps its `reading_order_index`**, which was computed against
                        // the panel it is leaving, so it can collide with one the adopting panel
                        // already uses (the reviewer's W4 round-2 N10). It is inert in M1: no rule
                        // validates dialogue index uniqueness (rule 6 covers panels) and no command
                        // or export consumes it, so the collision changes nothing that is read. It
                        // becomes a defect the day anything orders dialogues by it; renumbering here
                        // would be a schema decision (what is the order of the adopted record?)
                        // rather than an M1 fix, so it is recorded rather than repaired.
                        let oc = open_mut(s)?;
                        let (d_page, index) = locate_dialogue(&oc.bundle, existing)?;
                        page_file_mut(&mut oc.bundle, d_page)?.dialogue[index].panel_id = panel;
                    }
                    Some(existing)
                }
                None => {
                    let oc = open_mut(s)?;
                    let did = oc.bundle.allocate::<DialogueId>().map_err(|e| EngineError::Other(format!("{e}")))?;
                    let file = page_file_mut(&mut oc.bundle, page)?;
                    let index = u16::try_from(file.dialogue.iter().filter(|d| d.panel_id == panel).count()).unwrap_or(u16::MAX);
                    file.dialogue.push(Dialogue {
                        id: did,
                        panel_id: panel,
                        speaker_character_id: None,
                        text: words.clone(),
                        language: None,
                        reading_order_index: index,
                        approval_status: ApprovalStatus::Pending,
                        unknown: Default::default(),
                    });
                    Some(did)
                }
            }
        } else {
            None
        };
        let dashed = style.dashed;
        let (frame_node, tail_node, text_node) = s.edit("Add Lettering", |d, _| {
            let parent = d.default_layer();
            let mut f = Node::path(d.alloc_id(), plan.frame.clone(), outline_appearance(dashed));
            f.name = Some(format!("{} frame", kind_name(kind)));
            let fid = d.insert(parent, usize::MAX, f)?;
            let tail_node = match plan.tail_path() {
                Some(path) => {
                    let mut t = Node::path(d.alloc_id(), path.clone(), outline_appearance(false));
                    t.name = Some("Balloon tail".to_string());
                    Some(d.insert(parent, usize::MAX, t)?)
                }
                None => None,
            };
            let text = text_object(&words, plan.text_frame, &style.text, plan.fit);
            let mut tn = Node::new(d.alloc_id(), NodeKind::Text(Box::new(text)));
            tn.name = Some(format!("{} text", kind_name(kind)));
            let tn_id = d.insert(parent, usize::MAX, tn)?;
            Ok((fid, tail_node, tn_id))
        })?;
        {
            let oc = open_mut(s)?;
            let file = page_file_mut(&mut oc.bundle, page)?;
            file.lettering.push(Lettering {
                id,
                page_id: page,
                panel_id: panel,
                artboard_id,
                kind,
                dialogue_id,
                text: if kind.carries_own_text() { Some(words.clone()) } else { None },
                frame_node_id: frame_node,
                tail_node_id: tail_node,
                text_node_id: text_node,
                text_layout_settings: plan.text_layout.clone(),
                overflow_policy: plan.overflow_policy,
                unknown: Default::default(),
            });
            let pi = file.panels.iter().position(|p2| p2.id == panel).ok_or_else(|| EngineError::Other(format!("no such panel {}", panel.0)))?;
            file.panels[pi].lettering_ids.push(id);
            // The panel that owns the dialogue is the one that lists it (§6.1: one owner). A
            // balloon may *show* another panel's dialogue — that is one record and one renderer per
            // balloon, not two sources of truth — but it must not put itself on the owner's list.
            if let Some(did) = dialogue_id
                && owns
                && !file.panels[pi].dialogue_ids.contains(&did)
            {
                file.panels[pi].dialogue_ids.push(did);
            }
        }
        Ok((id, dialogue_id))
    })?;
    let (frame_node, tail_node, text_node) = {
        let b = &open(s)?.bundle;
        let (page, index) = locate_lettering(b, lettering_id)?;
        let l = &page_file(b, page)?.lettering[index];
        (l.frame_node_id, l.tail_node_id, l.text_node_id)
    };
    Ok(json!({
        "lettering": lettering_id.0,
        "dialogue": dialogue_id.map(|d| d.0),
        "kind": kind_name(kind),
        "frame": frame_node.0,
        "tail": tail_node.map(|t| t.0),
        "text_node": text_node.0,
        "bounds": rect_json(plan.bounds),
        "text_layout": layout_json(&plan.text_layout),
        "findings": plan.findings.iter().map(finding_json).collect::<Vec<_>>(),
    }))
}

// ---------- re-fit ----------

/// Re-fit a lettering under `policy`, writing the plan's geometry and typography and the record's
/// settings. `words` is what the record now owns.
fn refit(s: &mut Session, lettering_id: LetteringId, words: &str, style: &LetteringStyle, policy: OverflowPolicy) -> Result<Value> {
    let (kind, bounds) =
        {
            let b = &open(s)?.bundle;
            let (page, index) = locate_lettering(b, lettering_id)?;
            let l = &page_file(b, page)?.lettering[index];
            let bounds =
                s.doc()?.doc.node(l.frame_node_id).and_then(|n| n.geometric_bounds()).ok_or_else(|| {
                    EngineError::Other(format!("lettering {}'s frame {} has no geometry to re-fit", lettering_id.0, l.frame_node_id.0))
                })?;
            (l.kind, bounds)
        };
    let source = if kind.is_spoken() { TextSource::Dialogue(words) } else { TextSource::Own(words) };
    let request = LetteringRequest::new(kind, source).with_policy(policy).with_style(style.clone());
    let plan = refit_balloon(FontDb::global(), &request, bounds).map_err(|e| EngineError::Other(format!("{e}")))?;
    let frame_node = {
        let b = &open(s)?.bundle;
        let (page, index) = locate_lettering(b, lettering_id)?;
        page_file(b, page)?.lettering[index].frame_node_id
    };
    let touched = apply_to(s, frame_node, &plan, words, &style.text)?;
    {
        let oc = open_mut(s)?;
        let (page, index) = locate_lettering(&oc.bundle, lettering_id)?;
        let l = &mut page_file_mut(&mut oc.bundle, page)?.lettering[index];
        l.text_layout_settings = plan.text_layout.clone();
        l.overflow_policy = plan.overflow_policy;
    }
    Ok(json!({
        "lettering": lettering_id.0,
        "kind": kind_name(kind),
        "text": words,
        "text_layout": layout_json(&plan.text_layout),
        "overflow_policy": overflow_name(plan.overflow_policy),
        "bounds": rect_json(plan.bounds),
        "outcome": outcome_name(plan.outcome),
        "moved": touched,
        "findings": plan.findings.iter().map(finding_json).collect::<Vec<_>>(),
    }))
}

/// Apply a plan to the objects named by the lettering whose frame is `frame_node`.
fn apply_to(s: &mut Session, frame_node: NodeId, plan: &LetteringPlan, words: &str, style: &FitTextStyle) -> Result<Vec<u64>> {
    let (tail_node, text_node) = {
        let b = &open(s)?.bundle;
        let l = b
            .pages
            .values()
            .flat_map(|f| f.lettering.iter())
            .find(|l| l.frame_node_id == frame_node)
            .ok_or_else(|| EngineError::Other(format!("no lettering names frame {}", frame_node.0)))?;
        (l.tail_node_id, l.text_node_id)
    };
    let frame_path: PathData = plan.frame.clone();
    let tail_path = plan.tail_path().cloned();
    let text_frame = plan.text_frame;
    let fit = plan.fit;
    let style = style.clone();
    let words = words.to_string();
    s.edit("Re-fit Lettering", move |d, _| {
        let mut touched = vec![frame_node.0];
        if let Some(n) = d.node_mut(frame_node)
            && let NodeKind::Path { path, .. } = &mut n.kind
        {
            *path = frame_path;
        }
        match (tail_node, tail_path) {
            (Some(id), Some(path)) => {
                if let Some(n) = d.node_mut(id)
                    && let NodeKind::Path { path: p, .. } = &mut n.kind
                {
                    *p = path;
                }
                touched.push(id.0);
            }
            (Some(id), None) => {
                let _ = d.remove(id);
            }
            _ => {}
        }
        if let Some(n) = d.node_mut(text_node)
            && let NodeKind::Text(t) = &mut n.kind
        {
            let mut fresh = text_object(&words, text_frame, &style, fit);
            let xf = t.xf;
            fresh.xf = xf;
            **t = fresh;
        }
        touched.push(text_node.0);
        Ok(touched)
    })
}

// ---------- comic.lettering.set_text ----------

fn set_text(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "comic.lettering.set_text";
    let lettering_id = LetteringId(super::panel::raw_id(p, "lettering", C)?);
    let words = str_param(p, "text").ok_or_else(|| bad(C, "missing `text`"))?.to_string();
    let (kind, policy, style) = {
        let b = &open(s)?.bundle;
        let (page, index) = locate_lettering(b, lettering_id)?;
        let l = &page_file(b, page)?.lettering[index];
        (l.kind, l.overflow_policy, style_from_record(b, l))
    };
    comic_edit(s, "Set Lettering Text", |s| {
        if kind.is_spoken() {
            let oc = open_mut(s)?;
            let (page, index) = locate_lettering(&oc.bundle, lettering_id)?;
            let dialogue_id = page_file(&oc.bundle, page)?.lettering[index]
                .dialogue_id
                .ok_or_else(|| EngineError::Other(format!("lettering {} is spoken but names no dialogue (schema rule 8)", lettering_id.0)))?;
            let file = page_file_mut(&mut oc.bundle, page)?;
            // The record is looked up **on the balloon's own page**, and that is deliberate: the
            // cross-page form is legal but unspecified (no rule ties a `Lettering` to its
            // dialogue's page, and `comic.lettering.add` accepts it), so this command refuses
            // rather than reaching into another page's file for the words. `comic.dialogue.update`
            // resolves the record by id bundle-wide and is the verb that reaches such a balloon, so
            // the refusal is a documented asymmetry, not a dead end. See the params string.
            let d = file.dialogue.iter_mut().find(|d| d.id == dialogue_id).ok_or_else(|| {
                EngineError::Other(format!(
                    "lettering {} names dialogue {}, which is not on page {}: comic.lettering.set_text edits the record in the balloon's own page, and a dialogue owned by another page's panel belongs to that page — use comic.dialogue.update {{dialogue}} to change its words",
                    lettering_id.0, dialogue_id.0, page.0
                ))
            })?;
            d.text = words.clone();
        } else {
            let oc = open_mut(s)?;
            let (page, index) = locate_lettering(&oc.bundle, lettering_id)?;
            page_file_mut(&mut oc.bundle, page)?.lettering[index].text = Some(words.clone());
        }
        refit(s, lettering_id, &words, &style, policy)
    })
}

// ---------- comic.lettering.set_style ----------

fn set_style(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "comic.lettering.set_style";
    let lettering_id = LetteringId(super::panel::raw_id(p, "lettering", C)?);
    let (current_policy, words, mut style, current_layout) = {
        let b = &open(s)?.bundle;
        let (page, index) = locate_lettering(b, lettering_id)?;
        let l = &page_file(b, page)?.lettering[index];
        let words = match l.dialogue_id {
            Some(id) => b.pages.values().flat_map(|f| f.dialogue.iter()).find(|d| d.id == id).map(|d| d.text.clone()).unwrap_or_default(),
            None => l.text.clone().unwrap_or_default(),
        };
        (l.overflow_policy, words, style_from_record(b, l), l.text_layout_settings.clone())
    };
    let policy = if p.get("overflowPolicy").is_some() { overflow_param(p, C)? } else { current_policy };
    if let Some(font) = str_param(p, "font") {
        style.text.family = font.to_string();
    }
    if p.get("size").is_some_and(|v| !v.is_null()) {
        style.text.size = positive_from(&p["size"], "size", C)?;
    }
    if p.get("leading").is_some() {
        style.text.leading = match p.get("leading") {
            Some(Value::Null) => None,
            Some(v) => Some(positive_from(v, "leading", C)?),
            None => style.text.leading,
        };
    }
    if p.get("tracking").is_some_and(|v| !v.is_null()) {
        let v = p.get("tracking").and_then(Value::as_f64).ok_or_else(|| bad(C, "`tracking` must be a number"))?;
        if !v.is_finite() {
            return Err(bad(C, format!("`tracking` must be finite, not {v}")));
        }
        style.text.tracking = v;
    }
    if p.get("padding").is_some_and(|v| !v.is_null()) {
        let v = p.get("padding").and_then(Value::as_f64).ok_or_else(|| bad(C, "`padding` must be a number"))?;
        if !(v.is_finite() && v >= 0.0) {
            return Err(bad(C, format!("`padding` must be finite and not negative, not {v}")));
        }
        style.text.padding = v;
    }
    if p.get("align").is_some_and(|v| !v.is_null()) {
        style.text.align = align_param(p, C)?;
    }
    let fit_to_frame = match p.get("fitToFrame") {
        None | Some(Value::Null) => current_layout.fit_to_frame || policy == OverflowPolicy::Shrink,
        Some(v) => v.as_bool().ok_or_else(|| bad(C, "`fitToFrame` must be true or false"))?,
    };
    comic_edit(s, "Set Lettering Style", |s| {
        {
            let oc = open_mut(s)?;
            let (page, index) = locate_lettering(&oc.bundle, lettering_id)?;
            let l = &mut page_file_mut(&mut oc.bundle, page)?.lettering[index];
            l.text_layout_settings = layout_settings(&style.text, fit_to_frame);
            l.overflow_policy = policy;
        }
        refit(s, lettering_id, &words, &style, policy)
    })
}

fn positive_from(v: &Value, key: &str, cmd: &str) -> Result<f64> {
    let n = v.as_f64().ok_or_else(|| bad(cmd, format!("`{key}` must be a number")))?;
    if !(n.is_finite() && n > 0.0) {
        return Err(bad(cmd, format!("`{key}` must be a finite, positive number of points, not {n}")));
    }
    Ok(n)
}

// ---------- comic.dialogue.update ----------

fn dialogue_update(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "comic.dialogue.update";
    let dialogue_id = DialogueId(super::panel::raw_id(p, "dialogue", C)?);
    let new_text = str_param(p, "text").map(str::to_string);
    let approval = match str_param(p, "approval") {
        None => None,
        Some("pending") => Some(ApprovalStatus::Pending),
        Some("approved") => Some(ApprovalStatus::Approved),
        Some("rejected") => Some(ApprovalStatus::Rejected),
        Some(other) => return Err(bad(C, format!("`approval` is pending, approved or rejected, not {other:?}"))),
    };
    comic_edit(s, "Update Dialogue", |s| {
        let (panel, text, nodes) = {
            let oc = open_mut(s)?;
            let (page, index) = locate_dialogue(&oc.bundle, dialogue_id)?;
            let panel = page_file(&oc.bundle, page)?.dialogue[index].panel_id;
            let file = page_file_mut(&mut oc.bundle, page)?;
            let d = &mut file.dialogue[index];
            if let Some(text) = &new_text {
                d.text = text.clone();
            }
            if p.get("speaker").is_some() {
                d.speaker_character_id = match p.get("speaker") {
                    Some(Value::Null) | None => None,
                    Some(v) => Some(CharacterId(v.as_u64().ok_or_else(|| bad(C, "`speaker` is a CharacterId or null"))?)),
                };
            }
            if p.get("language").is_some() {
                d.language = match p.get("language") {
                    Some(Value::Null) => None,
                    Some(Value::String(s2)) => Some(s2.clone()),
                    _ => return Err(bad(C, "`language` is a BCP-47 string or null")),
                };
            }
            if let Some(v) = p.get("readingOrderIndex").filter(|v| !v.is_null()) {
                d.reading_order_index = u16::try_from(v.as_u64().ok_or_else(|| bad(C, "`readingOrderIndex` is a whole number"))?)
                    .map_err(|_| bad(C, "`readingOrderIndex` is out of range"))?;
            }
            if let Some(a) = approval {
                d.approval_status = a;
            }
            let text = d.text.clone();
            // The words belong to the Dialogue, so the page must show the record's text. Only the
            // *words* move: no frame, tail, text-layout setting or size is touched, which is what
            // "without touching artwork or typography" means (M1 acceptance test 5).
            //
            // **Every** balloon that shows this record is rewritten, not only the one in the panel
            // that owns it: one `Dialogue` may be shown by a balloon in another panel (design §6.1
            // allows one authoritative record with several renderers, which is what
            // `comic.lettering.add {dialogue}` makes), and reaching only the owner's balloon would
            // leave the others showing the previous words — a visual object as a second source of
            // dialogue truth.
            let b = &oc.bundle;
            let nodes: Vec<vectorcraft_doc::NodeId> =
                b.pages.values().flat_map(|f| f.lettering.iter()).filter(|l| l.dialogue_id == Some(dialogue_id)).map(|l| l.text_node_id).collect();
            (panel, text, nodes)
        };
        for node in nodes {
            let words = text.clone();
            s.edit("Update Dialogue", move |d, _| rewrite_text(d, node, &words))?;
        }
        let record = {
            let b = &open(s)?.bundle;
            let (page, index) = locate_dialogue(b, dialogue_id)?;
            page_file(b, page)?.dialogue[index].clone()
        };
        Ok(json!({
            "dialogue": record.id.0,
            "panel": panel.0,
            "text": record.text,
            "speaker": record.speaker_character_id.map(|c| c.0),
            "language": record.language,
            "readingOrderIndex": record.reading_order_index,
            "approval": match record.approval_status { ApprovalStatus::Pending => "pending", ApprovalStatus::Approved => "approved", ApprovalStatus::Rejected => "rejected" },
            "typographyTouched": false,
            "geometryTouched": false,
        }))
    })
}
