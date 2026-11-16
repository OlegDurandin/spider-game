//! Pointer capture stays on the table while Leptos replaces individual card nodes.
use crate::{
    Game, Selection,
    input::{self, Bounds, Dialog, Intent, Point},
    motion,
};
use leptos::prelude::*;
use std::cell::{Cell, RefCell};
use wasm_bindgen::{JsCast, closure::Closure};
use web_sys::{Element, HtmlElement, MouseEvent, PointerEvent};

#[derive(Clone, Copy)]
pub struct Controls {
    pub game: RwSignal<Game>,
    pub selection: RwSignal<Selection>,
    pub notice: RwSignal<String>,
    pub hover: RwSignal<Option<usize>>,
    pub dragging: RwSignal<bool>,
    pub dialog: RwSignal<Dialog>,
}
struct Source {
    id: u8,
}
struct Gesture {
    ticket: u64,
    pointer: i32,
    touch: bool,
    mode: Intent,
    start: Point,
    point: Point,
    started: f64,
    source: Selection,
    first_id: Option<u8>,
    table: Element,
    scroll: i32,
    controls: Controls,
    ghost: Option<Element>,
    cards: Vec<Source>,
}
thread_local! {
    static ACTIVE:RefCell<Option<Gesture>>=const{RefCell::new(None)};
    static TICKET:Cell<u64>=const{Cell::new(0)};
    static SUPPRESS_UNTIL:Cell<f64>=const{Cell::new(0.)};
}
fn document() -> web_sys::Document {
    web_sys::window().unwrap().document().unwrap()
}
fn bounds(el: &Element) -> Bounds {
    let r = el.get_bounding_client_rect();
    Bounds {
        left: r.left(),
        top: r.top(),
        right: r.right(),
        bottom: r.bottom(),
    }
}
fn point(ev: &PointerEvent) -> Point {
    Point {
        x: ev.client_x() as f64,
        y: ev.client_y() as f64,
    }
}
fn style(el: &Element) -> Option<web_sys::CssStyleDeclaration> {
    el.dyn_ref::<HtmlElement>().map(HtmlElement::style)
}
fn card(id: u8) -> Option<Element> {
    document()
        .query_selector(&format!(".columns .card[data-card-id='{id}']"))
        .ok()
        .flatten()
}
fn drop_column(g: &Gesture) -> Option<usize> {
    let nodes = document().query_selector_all(".columns .column").ok()?;
    let columns: Vec<_> = (0..nodes.length())
        .filter_map(|i| {
            let el = nodes.item(i)?.dyn_into::<Element>().ok()?;
            let r = el.get_bounding_client_rect();
            Some((r.left(), r.right()))
        })
        .collect();
    input::column_at(g.point, bounds(&g.table), &columns)
}
pub fn consume_click(event: &MouseEvent) -> bool {
    event.detail() > 0 && SUPPRESS_UNTIL.with(|t| js_sys::Date::now() < t.get())
}
pub fn cancel() {
    let Some(g) = ACTIVE.with(|active| active.borrow_mut().take()) else {
        return;
    };
    if g.mode != Intent::Pending {
        SUPPRESS_UNTIL.with(|t| t.set(js_sys::Date::now() + 350.));
    }
    if let Some(ghost) = g.ghost {
        ghost.remove();
    }
    for source in g.cards {
        if let Some(el) = card(source.id) {
            el.class_list().remove_1("drag-source").ok();
        }
    }
    g.controls.hover.set(None);
    g.controls.dragging.set(false);
    if g.table.has_pointer_capture(g.pointer) {
        g.table.release_pointer_capture(g.pointer).ok();
    }
}
fn create_ghost(g: &mut Gesture) -> bool {
    let Some((from, start)) = g.source else {
        return false;
    };
    let game = g.controls.game.get_untracked();
    if !game.selectable(from, start)
        || game.state.columns[from].get(start).map(|c| c.id) != g.first_id
    {
        return false;
    }
    let Some(first) = card(game.state.columns[from][start].id) else {
        return false;
    };
    let rect = first.get_bounding_client_rect();
    let Ok(ghost) = document().create_element("div") else {
        return false;
    };
    ghost.set_class_name("drag-stack");
    ghost.set_attribute("aria-hidden", "true").ok();
    let mut height = rect.height();
    for c in &game.state.columns[from][start..] {
        let Some(el) = card(c.id) else {
            continue;
        };
        let r = el.get_bounding_client_rect();
        let offset = r.top() - rect.top();
        height = offset + r.height();
        let Some(clone) = el
            .clone_node_with_deep(true)
            .ok()
            .and_then(|n| n.dyn_into::<Element>().ok())
        else {
            continue;
        };
        clone.remove_attribute("data-card-id").ok();
        clone.set_attribute("data-drag-id", &c.id.to_string()).ok();
        clone.set_attribute("disabled", "").ok();
        clone.set_attribute("tabindex", "-1").ok();
        clone
            .set_attribute(
                "style",
                &format!(
                    "left:0;top:{offset}px;width:{}px;height:{}px",
                    r.width(),
                    r.height()
                ),
            )
            .ok();
        ghost.append_child(&clone).ok();
        g.cards.push(Source { id: c.id });
    }
    ghost
        .set_attribute(
            "style",
            &format!(
                "left:{}px;top:{}px;width:{}px;height:{height}px",
                rect.left(),
                rect.top(),
                rect.width()
            ),
        )
        .ok();
    let Ok(Some(root)) = document().query_selector(".game") else {
        return false;
    };
    root.append_child(&ghost).ok();
    g.ghost = Some(ghost);
    true
}
fn update_visual(g: &Gesture) {
    let dx = g.point.x - g.start.x;
    let dy = g.point.y - g.start.y;
    if let Some(ghost) = &g.ghost {
        if let Some(s) = style(ghost) {
            s.set_property("transform", &format!("translate({dx}px,{dy}px)"))
                .ok();
        }
    }
    for source in &g.cards {
        if let Some(el) = card(source.id) {
            if !el.class_list().contains("drag-source") {
                el.class_list().add_1("drag-source").ok();
            }
        }
    }
    let candidate = drop_column(g);
    let target = g.controls.game.with_untracked(|game| {
        candidate.filter(|&to| {
            g.source
                .is_some_and(|(from, start)| game.legal(spider_engine::Move { from, start, to }))
        })
    });
    if g.controls.hover.get_untracked() != target {
        g.controls.hover.set(target);
    }
}
fn frame() {
    let callback = Closure::once_into_js(move |_: f64| {
        let running = ACTIVE.with(|active| {
            let mut active = active.borrow_mut();
            let Some(g) = active.as_mut() else {
                return false;
            };
            if g.mode != Intent::Drag {
                return false;
            }
            let delta = input::scroll_delta(g.point, bounds(&g.table));
            if delta != 0 {
                g.table.set_scroll_top(g.table.scroll_top() + delta);
            }
            update_visual(g);
            true
        });
        if running {
            frame();
        }
    });
    web_sys::window()
        .unwrap()
        .request_animation_frame(callback.unchecked_ref())
        .ok();
}
fn activate(ticket: u64) {
    let valid = ACTIVE.with(|active| {
        let mut active = active.borrow_mut();
        let Some(g) = active.as_mut() else {
            return false;
        };
        if g.ticket != ticket
            || g.mode != Intent::Pending
            || motion::busy()
            || g.controls.dialog.get_untracked() != Dialog::None
        {
            return false;
        }
        if !create_ghost(g) {
            return false;
        }
        // Capture before selecting: the selection causes Leptos to replace cards.
        g.table.set_pointer_capture(g.pointer).ok();
        g.mode = Intent::Drag;
        g.controls.selection.set(g.source);
        g.controls.dragging.set(true);
        g.controls.notice.set(String::new());
        update_visual(g);
        true
    });
    if valid {
        frame();
    }
}
pub fn pointer_down(event: PointerEvent, controls: Controls) {
    if !event.is_primary() {
        cancel();
        return;
    }
    if event.button() != 0
        || motion::busy()
        || controls.dialog.get_untracked() != Dialog::None
        || controls.game.get_untracked().state.won()
    {
        return;
    }
    let Some(target) = event.target().and_then(|t| t.dyn_into::<Element>().ok()) else {
        return;
    };
    let Ok(Some(el)) = target.closest(".card[data-card-id]") else {
        return;
    };
    let Some(id) = el
        .get_attribute("data-card-id")
        .and_then(|s| s.parse::<u8>().ok())
    else {
        return;
    };
    let Ok(Some(table)) = el.closest(".table-scroll") else {
        return;
    };
    cancel();
    SUPPRESS_UNTIL.with(|t| t.set(0.));
    let game = controls.game.get_untracked();
    let source = game.state.columns.iter().enumerate().find_map(|(from, c)| {
        c.iter()
            .position(|c| c.id == id)
            .filter(|&start| game.selectable(from, start))
            .map(|start| (from, start))
    });
    let ticket = TICKET.with(|t| {
        t.set(t.get().wrapping_add(1));
        t.get()
    });
    let touch = event.pointer_type() != "mouse";
    ACTIVE.with(|active| {
        *active.borrow_mut() = Some(Gesture {
            ticket,
            pointer: event.pointer_id(),
            touch,
            mode: Intent::Pending,
            start: point(&event),
            point: point(&event),
            started: js_sys::Date::now(),
            source,
            first_id: source.map(|_| id),
            scroll: table.scroll_top(),
            table,
            controls,
            ghost: None,
            cards: vec![],
        })
    });
    if touch && source.is_some() {
        let callback = Closure::once_into_js(move || {
            let pending = ACTIVE.with(|a| {
                a.borrow()
                    .as_ref()
                    .is_some_and(|g| g.ticket == ticket && g.mode == Intent::Pending)
            });
            if pending {
                activate(ticket);
            }
        });
        web_sys::window()
            .unwrap()
            .set_timeout_with_callback_and_timeout_and_arguments_0(
                callback.unchecked_ref(),
                input::HOLD_MS as i32,
            )
            .ok();
    }
}
fn pointer_move(event: PointerEvent) {
    let activate_ticket = ACTIVE.with(|active| {
        let mut active = active.borrow_mut();
        let Some(g) = active.as_mut() else {
            return None;
        };
        if g.pointer != event.pointer_id() {
            return None;
        }
        g.point = point(&event);
        let next = input::intent(
            g.mode,
            g.touch,
            g.source.is_some(),
            g.start,
            g.point,
            js_sys::Date::now() - g.started,
        );
        if g.mode == Intent::Pending && next == Intent::Drag {
            event.prevent_default();
            return Some(g.ticket);
        }
        if next == Intent::Scroll {
            if g.mode != Intent::Scroll {
                g.table.set_pointer_capture(g.pointer).ok();
            }
            g.mode = Intent::Scroll;
            event.prevent_default();
            g.table
                .set_scroll_top(g.scroll - (g.point.y - g.start.y).round() as i32);
        } else if g.mode == Intent::Drag {
            event.prevent_default();
            update_visual(g);
        }
        None
    });
    if let Some(ticket) = activate_ticket {
        activate(ticket);
    }
}
fn pointer_up(event: PointerEvent) {
    let drop = ACTIVE.with(|active| {
        let mut active = active.borrow_mut();
        let g = active.as_mut()?;
        if g.pointer != event.pointer_id() {
            return None;
        }
        if g.mode != Intent::Drag {
            return Some((g.controls, None));
        }
        event.prevent_default();
        g.point = point(&event);
        update_visual(g);
        Some((
            g.controls,
            g.source.and_then(|(from, start)| {
                drop_column(g).map(|to| spider_engine::Move { from, start, to })
            }),
        ))
    });
    let Some((controls, drop)) = drop else {
        return;
    };
    if controls.dialog.get_untracked() != Dialog::None {
        cancel();
        return;
    }
    if let Some(m) = drop.filter(|m| controls.game.get_untracked().legal(*m)) {
        // command snapshots the translated source positions, then cancels drag visuals.
        crate::command(controls.game, controls.selection, controls.notice, |g| {
            g.apply(m)
        });
    } else {
        if drop.is_some_and(|m| m.from != m.to) {
            controls.notice.set("Сюда нельзя положить эти карты".into());
        }
        cancel();
    }
}
pub fn install(controls: Controls) {
    let window = web_sys::window().unwrap();
    let moved = Closure::<dyn FnMut(PointerEvent)>::new(pointer_move);
    window
        .add_event_listener_with_callback("pointermove", moved.as_ref().unchecked_ref())
        .ok();
    moved.forget();
    let released = Closure::<dyn FnMut(PointerEvent)>::new(pointer_up);
    window
        .add_event_listener_with_callback("pointerup", released.as_ref().unchecked_ref())
        .ok();
    released.forget();
    let cancelled = Closure::<dyn FnMut(PointerEvent)>::new(move |ev: PointerEvent| {
        if ACTIVE.with(|a| {
            a.borrow()
                .as_ref()
                .is_some_and(|g| g.pointer == ev.pointer_id())
        }) {
            cancel();
        }
    });
    window
        .add_event_listener_with_callback("pointercancel", cancelled.as_ref().unchecked_ref())
        .ok();
    cancelled.forget();
    let blur = Closure::<dyn FnMut()>::new(cancel);
    window
        .add_event_listener_with_callback("blur", blur.as_ref().unchecked_ref())
        .ok();
    blur.forget();
    let visibility = Closure::<dyn FnMut()>::new(move || {
        if document().hidden() {
            cancel();
        }
    });
    document()
        .add_event_listener_with_callback("visibilitychange", visibility.as_ref().unchecked_ref())
        .ok();
    visibility.forget();
    let escape =
        Closure::<dyn FnMut(web_sys::KeyboardEvent)>::new(move |ev: web_sys::KeyboardEvent| {
            if ev.key() == "Escape" {
                cancel();
                if controls.dialog.get_untracked() != Dialog::Corrupt {
                    controls.dialog.set(Dialog::None);
                }
            }
        });
    window
        .add_event_listener_with_callback("keydown", escape.as_ref().unchecked_ref())
        .ok();
    escape.forget();
}
