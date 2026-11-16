use crate::{input::Dialog, persist_session};
use leptos::prelude::*;
use spider_engine::{
    Card, Mode, Session, label,
    tripeaks::{Command, center, row},
};
fn act(game: RwSignal<Session>, notice: RwSignal<String>, cmd: Command) {
    let Some(mut g) = game.get_untracked().tripeaks().cloned() else {
        return;
    };
    if let Err(e) = g.apply(cmd) {
        notice.set(e.into());
        return;
    }
    let s = Session::Tripeaks(g);
    notice.set(if persist_session(&s).is_err() {
        "В этой сессии прогресс не сохраняется".into()
    } else {
        String::new()
    });
    game.set(s);
}
#[component]
fn Face(
    card: Card,
    name: String,
    style: String,
    hint: bool,
    disabled: bool,
    position: usize,
    on_tap: Callback<()>,
) -> impl IntoView {
    let rank = label(card.rank());
    let suit = card.suit(Mode::Four);
    let face = if (2..=10).contains(&card.rank()) {
        format!("cards/numeric-{}.svg", suit.code())
    } else {
        format!("cards/art-v2/{}{}.svg", rank, suit.code())
    };
    view! {<button class="card t-card" class:back=!card.open class:red=suit.red() class:ten=(card.rank()==10) class:t-hint=hint style=style disabled=disabled aria-label=name data-position=position data-card-id=card.id on:click=move |_|on_tap.run(())>
      {card.open.then(||view!{<img class="card-face" src=face alt="" draggable="false"/><span class="rank">{rank}<small>{suit.symbol()}</small></span>})}
    </button>}
}
#[component]
pub fn Board(
    game: RwSignal<Session>,
    notice: RwSignal<String>,
    dialog: RwSignal<Dialog>,
) -> impl IntoView {
    let hint = RwSignal::new(None::<Command>);
    Effect::new(move |_| {
        game.track();
        hint.set(None);
    });
    view! {<section class="tripeaks" aria-label="Три пика">
      <section class="stats"><span>"Ходы "<b>{move||game.get().moves()}</b></span><span>"Снято "<b>{move||game.get().tripeaks().map_or(0,|g|g.state.tableau.iter().filter(|c|c.is_none()).count())}"/28"</b></span></section>
      <p class="t-intro">"Снимай карты на ранг выше или ниже верхней карты сброса"</p>
      <div class="t-scroll"><div class="t-layout">
      <section class="t-table" aria-label="Три вершины">
       {move||{let Some(g)=game.get().tripeaks().cloned()else{return ().into_any()};let h=hint.get();
        g.state.tableau.iter().enumerate().filter_map(|(i,c)|c.map(|card|{
          let name=if card.open{format!("{} {}, позиция {}",label(card.rank()),card.suit(Mode::Four).name(),i+1)}else{format!("Закрытая карта, позиция {}",i+1)};
          view!{<Face card=card name=name position=i style=format!("left:calc(var(--step) * {});top:calc(var(--row-step) * {})",center(i),row(i)) hint={h==Some(Command::Remove(i))} disabled={!card.open} on_tap=Callback::new(move |_|act(game,notice,Command::Remove(i)))/>}
        })).collect_view().into_any()
       }}
      </section>
      <section class="t-piles" aria-label="Запас и сброс">
       {move||{let Some(g)=game.get().tripeaks().cloned()else{return ().into_any()};let n=g.state.stock.len();let card=*g.state.waste.last().unwrap();
        view!{
         <div class="t-pile"><button class="card t-stock" class:back=(n>0) class:t-hint=(hint.get()==Some(Command::Draw)) disabled={n==0 || g.state.status!=spider_engine::tripeaks::Status::Playing} aria-label=format!("Взять из запаса. Осталось {n}") on:click=move |_|act(game,notice,Command::Draw)><span class="stock-count">{n}</span></button><span class="t-caption">"Запас "{n}</span></div>
         <div class="t-pile"><Face card=card name=format!("Сброс: {} {}",label(card.rank()),card.suit(Mode::Four).name()) position=28 style="".into() hint=false disabled=false on_tap=Callback::new(|_|{})/><span class="t-caption">"Сброс"</span></div>
        }.into_any()
       }}
      </section>
      </div></div>
      <section class="k-action" aria-live="polite"><p>{move||game.get().tripeaks().map(|g|format!("Цепочка: {} · Лучшая: {}",g.state.chain,g.state.best_chain))}</p><p class="notice">{move||notice.get()}</p></section>
      <footer class="k-footer"><button disabled=move||game.get().history_len()==0 on:click=move |_|{let mut s=game.get_untracked();if s.undo(){notice.set(if persist_session(&s).is_err(){"Не удалось сохранить партию".into()}else{String::new()});game.set(s);}}><span class="tool-icon">"↶"</span>"Отменить"</button>
      <button on:click=move |_|{hint.set(game.get_untracked().tripeaks().and_then(|g|g.hint()));notice.set(String::new());}><span class="tool-icon bulb" aria-hidden="true"></span>"Подсказка"</button></footer>
      <span class="sr-only">{move||format!("{:?}",dialog.get())}</span>
    </section>}
}
