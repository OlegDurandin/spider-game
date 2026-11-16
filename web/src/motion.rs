//! Visual effects run after the engine has committed and saved its decision.
use leptos::prelude::*;
use spider_engine::State;
use std::{
    cell::Cell,
    collections::{HashMap, HashSet},
};
use wasm_bindgen::{JsCast, closure::Closure};
use web_sys::Element;

thread_local! {
    static BUSY: Cell<bool> = const { Cell::new(false) };
    static STATUS: Cell<Option<RwSignal<bool>>> = const { Cell::new(None) };
    static GENERATION: Cell<u64> = const { Cell::new(0) };
}
#[derive(Clone, Copy, Default)]
struct Rect {
    x: f64,
    y: f64,
    w: f64,
    h: f64,
}
impl Rect {
    fn of(el: &Element) -> Self {
        let r = el.get_bounding_client_rect();
        Self {
            x: r.x(),
            y: r.y(),
            w: r.width(),
            h: r.height(),
        }
    }
}
struct Visual {
    rect: Rect,
    ghost: Element,
    open: bool,
}
pub struct Snapshot {
    cards: HashMap<u8, Visual>,
    deck: Rect,
    collected: Rect,
    completed: HashSet<u8>,
    columns: HashMap<u8, usize>,
}
fn document() -> web_sys::Document {
    web_sys::window().unwrap().document().unwrap()
}
fn elements(selector: &str) -> Vec<Element> {
    document()
        .query_selector_all(selector)
        .ok()
        .map(|nodes| {
            (0..nodes.length())
                .filter_map(|i| nodes.item(i)?.dyn_into::<Element>().ok())
                .collect()
        })
        .unwrap_or_default()
}
fn rect(selector: &str) -> Rect {
    document()
        .query_selector(selector)
        .ok()
        .flatten()
        .map(|el| Rect::of(&el))
        .unwrap_or_default()
}
fn card_id(el: &Element) -> Option<u8> {
    el.get_attribute("data-card-id")?.parse().ok()
}
pub fn attach(status: RwSignal<bool>) {
    STATUS.with(|s| s.set(Some(status)));
    let visibility = Closure::<dyn FnMut()>::new(|| {
        if document().hidden() {
            cancel();
        }
    });
    document()
        .add_event_listener_with_callback("visibilitychange", visibility.as_ref().unchecked_ref())
        .ok();
    visibility.forget();
}
fn set_busy(value: bool) {
    BUSY.with(|b| b.set(value));
    STATUS.with(|s| {
        if let Some(status) = s.get() {
            status.set(value);
        }
    });
}
pub fn busy() -> bool {
    BUSY.with(Cell::get)
}
pub fn cancel() {
    GENERATION.with(|g| g.set(g.get().wrapping_add(1)));
    set_busy(false);
    for el in elements(".flight-card") {
        el.remove();
    }
    for el in elements(".motion-hidden,.motion-active") {
        el.class_list()
            .remove_2("motion-hidden", "motion-active")
            .ok();
    }
}
pub fn capture(state: &State) -> Snapshot {
    // Drag ghosts are outside the scroll container; their positions are the
    // visual starting points for a drop animation. Original cards stay in layout.
    let dragged: HashMap<_, _> = elements(".drag-stack [data-drag-id]")
        .into_iter()
        .filter_map(|el| {
            Some((
                el.get_attribute("data-drag-id")?.parse::<u8>().ok()?,
                Rect::of(&el),
            ))
        })
        .collect();
    let cards = elements(".columns .card[data-card-id]")
        .into_iter()
        .filter_map(|el| {
            let id = card_id(&el)?;
            Some((
                id,
                Visual {
                    rect: dragged.get(&id).copied().unwrap_or_else(|| Rect::of(&el)),
                    open: !el.class_name().contains("back"),
                    ghost: {
                        let ghost: Element = el.clone_node_with_deep(true).ok()?.dyn_into().ok()?;
                        ghost.class_list().remove_1("drag-source").ok();
                        ghost
                    },
                },
            ))
        })
        .collect();
    Snapshot {
        cards,
        deck: rect(".deal-button"),
        collected: rect(".collected-count"),
        completed: state.completed.iter().flatten().map(|c| c.id).collect(),
        columns: state
            .columns
            .iter()
            .enumerate()
            .flat_map(|(col, cards)| cards.iter().map(move |c| (c.id, col)))
            .collect(),
    }
}
fn reduced() -> bool {
    web_sys::window()
        .unwrap()
        .match_media("(prefers-reduced-motion: reduce)")
        .ok()
        .flatten()
        .is_some_and(|m| m.matches())
}
fn animate(el: &Element, frames: &[(&str, f64)], duration: f64, delay: f64) {
    let keyframes = js_sys::Array::new();
    for (transform, opacity) in frames {
        let frame = js_sys::Object::new();
        js_sys::Reflect::set(&frame, &"transform".into(), &(*transform).into()).ok();
        js_sys::Reflect::set(&frame, &"opacity".into(), &(*opacity).into()).ok();
        keyframes.push(&frame);
    }
    let options = js_sys::Object::new();
    for (name, value) in [("duration", duration), ("delay", delay)] {
        js_sys::Reflect::set(&options, &name.into(), &value.into()).ok();
    }
    js_sys::Reflect::set(
        &options,
        &"easing".into(),
        &"cubic-bezier(.2,.75,.2,1)".into(),
    )
    .ok();
    js_sys::Reflect::set(&options, &"fill".into(), &"both".into()).ok();
    if let Ok(f) =
        js_sys::Reflect::get(el, &"animate".into()).and_then(|f| f.dyn_into::<js_sys::Function>())
    {
        f.call2(el, &keyframes, &options).ok();
    }
}
fn translated(from: Rect, to: Rect) -> String {
    format!("translate({}px,{}px)", from.x - to.x, from.y - to.y)
}
fn centered(source: Rect, target: Rect) -> Rect {
    Rect {
        x: source.x + source.w / 2. - target.w / 2.,
        y: source.y + source.h / 2. - target.h / 2.,
        ..target
    }
}
// Flight copies live outside the scroll container. Transforms on real cards
// expand its scrollable overflow and let the browser move the entire board.
fn flight(el: &Element, at: Rect) -> Option<Element> {
    let ghost: Element = el.clone_node_with_deep(true).ok()?.dyn_into().ok()?;
    ghost
        .class_list()
        .remove_2("motion-hidden", "drag-source")
        .ok();
    ghost.class_list().add_1("flight-card").ok();
    ghost.remove_attribute("data-card-id").ok();
    ghost.set_attribute("aria-hidden", "true").ok();
    ghost.set_attribute("disabled", "").ok();
    ghost.set_attribute("tabindex", "-1").ok();
    ghost.set_attribute("style",&format!("position:fixed;left:{}px;top:{}px;width:{}px;height:{}px;z-index:7;pointer-events:none",at.x,at.y,at.w,at.h)).ok();
    document()
        .query_selector(".game")
        .ok()??
        .append_child(&ghost)
        .ok()?;
    Some(ghost)
}
pub fn play(before: Snapshot, after: State) {
    if reduced() || document().hidden() {
        return;
    }
    set_busy(true);
    let generation = GENERATION.with(|g| {
        g.set(g.get().wrapping_add(1));
        g.get()
    });
    let frame = Closure::once_into_js(move |_: f64| {
        if GENERATION.with(Cell::get) != generation {
            return;
        }
        let mut ghosts = vec![];
        let mut moving = vec![];
        let mut longest: f64 = 0.;
        let current: HashMap<_, _> = elements(".columns .card[data-card-id]")
            .into_iter()
            .filter_map(|el| Some((card_id(&el)?, el)))
            .collect();
        // HashMap iteration is random. Append flight cards in their actual
        // column order so each lower card covers the card immediately above it.
        for (column, card) in after
            .columns
            .iter()
            .enumerate()
            .flat_map(|(column, cards)| cards.iter().map(move |card| (column, card)))
        {
            let id = &card.id;
            let Some(el) = current.get(id) else { continue };
            let to = Rect::of(el);
            let (from, delay) = if let Some(old) = before.cards.get(id) {
                (old.rect, 0.)
            } else {
                let from_collection = before.completed.contains(id);
                // Newly visible IDs arrive from the stock (or the collected pile on undo).
                let source = if from_collection {
                    before.collected
                } else {
                    before.deck
                };
                (centered(source, to), column as f64 * 35.)
            };
            // Geometry can change because of status text, focus or viewport changes.
            // Animate game transitions only; never animate an entire stationary board.
            let relocated = before
                .columns
                .get(id)
                .is_none_or(|old_column| *old_column != column);
            if relocated && ((from.x - to.x).abs() > 1. || (from.y - to.y).abs() > 1.) {
                let start = translated(from, to);
                if let Some(ghost) = flight(el, to) {
                    ghost.set_attribute("data-flight-id", &id.to_string()).ok();
                    el.class_list().add_1("motion-hidden").ok();
                    animate(&ghost, &[(&start, 1.), ("none", 1.)], 300., delay);
                    moving.push(el.clone());
                    ghosts.push(ghost);
                    longest = longest.max(300. + delay);
                }
            } else if before.cards.get(id).is_some_and(|v| !v.open)
                && !el.class_name().contains("back")
            {
                animate(
                    el,
                    &[("rotateY(-85deg)", 0.55), ("rotateY(0)", 1.)],
                    240.,
                    0.,
                );
                longest = longest.max(240.);
            }
        }
        if let Ok(Some(table)) = document().query_selector(".game") {
            for (n, (id, old)) in before
                .cards
                .into_iter()
                .filter(|(id, _)| !current.contains_key(id))
                .enumerate()
            {
                let collected = after.completed.iter().flatten().any(|c| c.id == id);
                let target = if collected {
                    before.collected
                } else {
                    before.deck
                };
                let el = old.ghost;
                el.set_class_name(&format!("{} flight-card", el.class_name()));
                el.remove_attribute("data-card-id").ok();
                el.set_attribute("aria-hidden", "true").ok();
                el.set_attribute("disabled", "").ok();
                el.set_attribute("style",&format!("position:fixed;left:{}px;top:{}px;width:{}px;height:{}px;z-index:7;pointer-events:none",old.rect.x,old.rect.y,old.rect.w,old.rect.h)).ok();
                table.append_child(&el).ok();
                let dest = centered(target, old.rect);
                let end = format!(
                    "translate({}px,{}px) scale(.16) rotate(12deg)",
                    dest.x - old.rect.x,
                    dest.y - old.rect.y
                );
                let delay = if collected {
                    80. + n as f64 * 14.
                } else {
                    n as f64 * 25.
                };
                animate(&el, &[("none", 1.), (&end, 0.)], 380., delay);
                longest = longest.max(380. + delay);
                ghosts.push(el);
            }
        }
        let cleanup = Closure::once_into_js(move || {
            for el in ghosts {
                el.remove();
            }
            if GENERATION.with(Cell::get) == generation {
                for el in moving {
                    el.class_list().remove_1("motion-hidden").ok();
                }
                set_busy(false);
            }
        });
        if web_sys::window()
            .unwrap()
            .set_timeout_with_callback_and_timeout_and_arguments_0(
                cleanup.unchecked_ref(),
                longest.ceil() as i32 + 30,
            )
            .is_err()
        {
            cancel();
        }
    });
    if web_sys::window()
        .unwrap()
        .request_animation_frame(frame.unchecked_ref())
        .is_err()
    {
        set_busy(false);
    }
}
