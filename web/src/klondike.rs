//! Klondike's board uses only targets and commands supplied by its Rust engine.
use crate::{input::Dialog, persist_session};
use leptos::prelude::*;
use spider_engine::{
    Card, Mode, Session,
    klondike::{Command, Game, Source, Target},
    label,
};
fn current(game: RwSignal<Session>) -> Option<Game> {
    game.get_untracked().klondike().cloned()
}
fn commit(game: RwSignal<Session>, notice: RwSignal<String>, g: Game) {
    let s = Session::Klondike(g);
    notice.set(if persist_session(&s).is_err() {
        "В этой сессии прогресс не сохраняется".into()
    } else {
        String::new()
    });
    game.set(s);
}
fn act(game: RwSignal<Session>, notice: RwSignal<String>, cmd: Command) {
    if let Some(mut g) = current(game) {
        match g.apply(cmd) {
            Ok(()) => commit(game, notice, g),
            Err(e) => notice.set(e.into()),
        }
    }
}
fn tap(
    game: RwSignal<Session>,
    selection: RwSignal<Option<Source>>,
    notice: RwSignal<String>,
    source: Option<Source>,
    target: Option<Target>,
) {
    let Some(g) = current(game) else { return };
    if let (Some(from), Some(to)) = (selection.get_untracked(), target) {
        let command = Command::Move { from, to };
        if g.legal(command) {
            act(game, notice, command);
            selection.set(None);
            return;
        }
    }
    if selection.get_untracked() == source {
        selection.set(None);
        notice.set(String::new());
        return;
    }
    if let Some(from) = source.filter(|from| g.selectable(*from)) {
        selection.set(Some(from));
        notice.set(String::new());
    } else {
        selection.set(None);
        notice.set(
            if source.is_some() {
                "Выберите открытый хвост: по порядку и с чередованием цветов"
            } else {
                "Пустой столбец принимает только короля, дом — следующую карту своей масти"
            }
            .into(),
        );
    }
}
#[component]
fn Face(card: Card, top: i32, name: String, selected: bool, on_tap: Callback<()>) -> impl IntoView {
    let rank = label(card.rank());
    let suit = card.suit(Mode::Four);
    let face = if (2..=10).contains(&card.rank()) {
        format!("cards/numeric-{}.svg", suit.code())
    } else {
        format!("cards/art-v2/{}{}.svg", rank, suit.code())
    };
    view! {<button class="card k-card" class:back=!card.open class:red=suit.red() class:ten=(card.rank()==10) class:k-selected=selected style=format!("top:calc(var(--unit) * {top})") data-card-id=card.id aria-label=name draggable="false" on:click=move|e|{e.stop_propagation();on_tap.run(())}>
       {card.open.then(||view!{<img class="card-face" src=face alt="" draggable="false"/><span class="rank">{rank}<small>{suit.symbol()}</small></span>})}
    </button>}
}
#[component]
pub fn Board(
    game: RwSignal<Session>,
    notice: RwSignal<String>,
    dialog: RwSignal<Dialog>,
) -> impl IntoView {
    let selected = RwSignal::new(None::<Source>);
    let stock_hint = RwSignal::new(false);
    Effect::new(move |_| {
        game.track();
        selected.set(None);
        stock_hint.set(false);
    });
    view! {<section class="klondike" aria-label="Косынка">
      <section class="stats" aria-label="Счёт Косынки"><span>"Ходы "<b>{move||game.get().moves()}</b></span><span>"В домах "{move||game.get().klondike().map_or(0,|g|g.state.collected())}"/52"</span></section>
      <div class="k-piles" aria-label="Запас, сброс и дома">
       {move||{
        let Some(g)=game.get().klondike().cloned()else{return ().into_any()};
        let source=selected.get();let targets=source.map(|s|g.targets(s)).unwrap_or_default();
        let stock_empty=g.state.stock.is_empty();let stock_disabled=stock_empty&&g.state.waste.is_empty();
        let stock_label=if stock_empty{if stock_disabled{"Запас и сброс пусты".into()}else{"Перевернуть сброс в запас".into()}}else{format!("Открыть одну карту из запаса. Осталось {}",g.state.stock.len())};
        let waste=g.state.waste.last().copied();
        view!{
         <div class="k-pile"><button class="card k-stock" class:back=!stock_empty class:target=move||stock_hint.get() disabled=stock_disabled aria-label=stock_label on:click=move |_|{stock_hint.set(false);selected.set(None);act(game,notice,Command::Draw)}>
           {if stock_empty{view!{<span class="stock-recycle" aria-hidden="true">{if stock_disabled{"—"}else{"↻"}}</span>}.into_any()}else{view!{<span class="stock-count">{g.state.stock.len()}</span>}.into_any()}}
         </button></div>
         <div class="k-pile" aria-label="Сброс">{waste.map(|card|view!{<Face card=card top=0 name=format!("Сброс: {} {}",label(card.rank()),card.suit(Mode::Four).name()) selected={source==Some(Source::Waste)} on_tap=Callback::new(move|_|tap(game,selected,notice,Some(Source::Waste),None))/>})}</div>
         <div class="k-gap" aria-hidden="true"></div>
         {(0..4).map(|index|{
          let suit=Card{id:index as u8*13,open:true}.suit(Mode::Four);let target=Target::Foundation(index);let available=targets.contains(&target);let card=g.state.foundations[index].last().copied();
          view!{<div class="k-pile k-foundation" class:target=available data-foundation=index>
           {if let Some(card)=card{view!{<Face card=card top=0 name=format!("Дом {}: {}",suit.name(),label(card.rank())) selected={source==Some(Source::Foundation{index})} on_tap=Callback::new(move|_|tap(game,selected,notice,Some(Source::Foundation{index}),Some(target)))/>}.into_any()}
           else{view!{<button class="k-empty" class:red=suit.red() aria-label=format!("Пустой дом {}",suit.name()) on:click=move |_|tap(game,selected,notice,None,Some(target))>{suit.symbol()}</button>}.into_any()}}
           {available.then(||view!{<span class="target-mark" aria-hidden="true">"↓"</span>})}
          </div>}
         }).collect_view()}
        }.into_any()
       }}
      </div>
      <section class="table-scroll k-table" aria-label="Стол Косынки" on:contextmenu=move|e|e.prevent_default()>
       <div class="k-columns">
        {move||{
          let Some(g)=game.get().klondike().cloned()else{return ().into_any()};let source=selected.get();let targets=source.map(|s|g.targets(s)).unwrap_or_default();
          g.state.columns.iter().enumerate().map(|(column,cards)|{
            let mut y=0;let positions:Vec<_>=cards.iter().map(|c|{let top=y;y+=if c.open{34}else{14};top}).collect();let height=positions.last().copied().unwrap_or(0)+116;
            let start=if let Some(Source::Tableau{column:c,start})=source{(c==column).then_some(start)}else{None};
            let target=Target::Tableau(column);let available=targets.contains(&target);
            view!{<div class="column k-column" class:target=available style=format!("height:calc(var(--unit) * {height})") data-column=column>
              <button class="k-empty" aria-label=format!("Столбец {}, пустое место для короля",column+1) on:click=move |_|tap(game,selected,notice,None,Some(target))>"K"</button>
              {available.then(||view!{<span class="target-mark" aria-hidden="true">"↓"</span>})}
              {cards.iter().enumerate().map(|(index,card)|{
                let card=*card;let accessible=if card.open{format!("{} {}, столбец {}",label(card.rank()),card.suit(Mode::Four).name(),column+1)}else{format!("Закрытая карта, столбец {}",column+1)};
                view!{<Face card=card top=positions[index] name=accessible selected=false on_tap=Callback::new(move|_|tap(game,selected,notice,Some(Source::Tableau{column,start:index}),Some(target)))/>}
              }).collect_view()}
              {start.filter(|&s|s<positions.len()).map(|s|view!{<div class="selection-ring" style=format!("top:calc(var(--unit)*{});height:calc(var(--unit)*{})",positions[s],height-positions[s])></div>})}
            </div>}
          }).collect_view().into_any()
        }}
       </div>
      </section>
      <section class="k-action" aria-live="polite"><p>{move||if selected.get().is_some(){"Выберите подсвеченный столбец или дом"}else{"Коснитесь карты, чтобы выбрать ход"}}</p><p class="notice">{move||notice.get()}</p></section>
      <footer class="k-footer">
       <button disabled=move||game.get().history_len()==0 on:click=move |_|{if let Some(mut g)=current(game){if g.undo(){commit(game,notice,g);}}}><span class="tool-icon">"↶"</span><span>"Отменить"</span></button>
       <button on:click=move |_|{if let Some(g)=current(game){match g.hint(){Some(Command::Move{from,to})=>{selected.set(Some(from));stock_hint.set(false);notice.set(match to {Target::Tableau(column)=>format!("Подсказка: перенесите в столбец {}",column+1),Target::Foundation(index)=>format!("Подсказка: перенесите в дом {}",Card{id:index as u8*13,open:true}.suit(Mode::Four).name())});},Some(Command::Draw)=>{selected.set(None);stock_hint.set(true);notice.set(if g.state.stock.is_empty(){"Переверните сброс в запас"}else{"Откройте следующую карту из запаса"}.into());},None=>{selected.set(None);stock_hint.set(false);notice.set("Полезных перестановок не найдено. Можно отменить ход или попробовать другой план".into());}}}}><span class="tool-icon bulb">"♧"</span><span>"Подсказка"</span></button>
      </footer>
      <span class="sr-only">{move||if dialog.get()!=Dialog::None{"Открыто меню игры"}else{""}}</span>
    </section>}
}
