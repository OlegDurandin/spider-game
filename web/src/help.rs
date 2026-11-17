//! Bundled rules. The help context is captured at entry, independently of the table.
use crate::input::Dialog;
use leptos::prelude::*;
use spider_engine::{Card, Kind, Mode, label};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Context {
    pub kind: Kind,
    pub mode: Mode,
    pub return_to: Dialog,
}
impl Context {
    pub fn capture(
        current: Kind,
        mode: Mode,
        selected: Kind,
        selected_mode: Mode,
        from: Dialog,
    ) -> Self {
        if from == Dialog::ConfirmNew {
            Self {
                kind: selected,
                mode: selected_mode,
                return_to: from,
            }
        } else {
            Self {
                kind: current,
                mode,
                return_to: from,
            }
        }
    }
    pub fn return_label(self) -> &'static str {
        match self.return_to {
            Dialog::ConfirmNew => "Назад к выбору",
            Dialog::Actions => "Назад к действиям",
            _ => "Вернуться к игре",
        }
    }
    pub fn title(self) -> String {
        match self.kind {
            Kind::Spider => format!("Паук · {}", self.mode.label()),
            Kind::Klondike => "Косынка · По одной карте".into(),
            Kind::Tripeaks => "Три пика".into(),
        }
    }
}
struct Rules {
    goal: &'static str,
    action: &'static str,
    example: &'static str,
    questions: &'static [(&'static str, &'static str)],
}
fn rules(context: Context) -> Rules {
    match context.kind {
        Kind::Spider => Rules {
            goal: "Соберите восемь рядов одной масти от короля до туза. Каждый готовый ряд сам уйдёт со стола.",
            action: "Коснитесь открытой карты, затем подсвеченного столбца. На свободную восьмёрку можно положить семёрку. Несколько карт переносятся вместе, если идут подряд по убыванию и имеют одну масть.",
            example: if context.mode == Mode::One {
                "Коснитесь 7♠ в ряду 7♠–6♠–5♠, затем свободной 8♠. Все три карты переедут вместе."
            } else {
                "На 8♠ можно положить 7♥. Семёрку можно будет снять отдельно. Поднять эти две карты вместе не получится: у них разные масти."
            },
            questions: &[
                (
                    "Что можно положить в пустой столбец?",
                    "Любую свободную карту или подходящий для переноса ряд. Освободив закрытую карту, вы сразу увидите её лицевую сторону.",
                ),
                (
                    "Почему не раздаётся новый ряд?",
                    "Для раздачи в каждом из десяти столбцов должна лежать хотя бы одна карта. Заполните пустой столбец, затем нажмите «Раздать ряд». Если запас закончился, продолжайте ходами со стола.",
                ),
            ],
        },
        Kind::Klondike => Rules {
            goal: "Переложите все 52 карты в четыре верхние стопки — «дома». В каждом доме собирается одна масть от туза до короля.",
            action: "На столе кладите младшую карту на старшую, чередуя красный и чёрный цвета. Коснитесь карты, затем места назначения. Можно перенести сразу несколько открытых карт, если они уже лежат подряд в таком порядке.",
            example: "Перенесите красную 7♥ на чёрную 8♣. Если под семёркой лежит чёрная 6♠, она переедет вместе с ней.",
            questions: &[
                (
                    "С чего начать дом?",
                    "Положите туз на место с его мастью. На A♥ идёт 2♥, затем 3♥. В дом можно отправить свободную карту со стола или верхнюю карту сброса. Верхнюю карту дома можно вернуть на стол по обычным правилам.",
                ),
                (
                    "Какая карта войдёт в пустой столбец?",
                    "Король или ряд, который начинается с короля. Когда освободится закрытая карта, она откроется.",
                ),
                (
                    "Как пользоваться колодой?",
                    "Коснитесь закрытой колоды, чтобы открыть одну карту в соседнюю стопку — сброс. Доступна верхняя карта сброса. Когда колода закончится, коснитесь её пустого места: оставшиеся карты перевернутся для нового прохода. Колоду можно проходить сколько угодно раз.",
                ),
            ],
        },
        Kind::Tripeaks => Rules {
            goal: "Уберите с трёх вершин все 28 карт.",
            action: "Посмотрите на открытую карту рядом с запасом. Коснитесь свободной карты на вершинах, которая на один ранг старше или младше. Подойдёт любая масть. Снятая карта подскажет, что можно взять дальше.",
            example: "Если в сбросе 8♣, можно снять 7♥. Теперь сверху семёрка, и на неё подойдёт 6♦ или свободная восьмёрка.",
            questions: &[
                (
                    "Когда откроется закрытая карта?",
                    "Когда вы уберёте обе карты, которые её перекрывают.",
                ),
                (
                    "Что положить на туза?",
                    "Двойку или короля. С короля тоже можно перейти на туза.",
                ),
                (
                    "Что делать, если подходящей карты нет?",
                    "Коснитесь запаса, чтобы открыть следующую карту. Запас пролистывается один раз. Даже с пустым запасом можно снимать подходящие карты со стола. Когда запас пуст и ходов со стола больше нет, можно отменить ход и попробовать другой путь.",
                ),
            ],
        },
    }
}
// Static cards share the exact SVG faces, indices and suit mapping used by the table.
#[component]
fn ExampleCard(id: u8, offset: usize) -> impl IntoView {
    let card = Card { id, open: true };
    let rank = label(card.rank());
    let suit = card.suit(Mode::Four);
    let face = if (2..=10).contains(&card.rank()) {
        format!("cards/numeric-{}.svg", suit.code())
    } else {
        format!("cards/art-v2/{}{}.svg", rank, suit.code())
    };
    view! {<div class="card help-card" class:red=suit.red() style=format!("top:{}px",offset*27) role="img" aria-label=format!("{} {}",rank,suit.name())>
        <img class="card-face" src=face alt=""/><span class="rank">{rank.clone()}<small>{suit.symbol()}</small></span>
    </div>}
}
#[component]
fn Example(context: Context) -> impl IntoView {
    if context.kind == Kind::Tripeaks {
        return view!{<figure class="help-example"><div class="help-cards help-sequence" aria-label="Сброс и последовательность снятия карт">
            <div class="help-stack"><ExampleCard id=33 offset=0/></div><span class="help-arrow" aria-hidden="true">"→"</span>
            <div class="help-stack"><ExampleCard id=19 offset=0/></div><span class="help-arrow" aria-hidden="true">"→"</span>
            <div class="help-stack"><ExampleCard id=44 offset=0/></div>
        </div><figcaption>{rules(context).example}</figcaption></figure>}.into_any();
    }
    let (destination, group, grouped): (u8, Vec<u8>, bool) = match context.kind {
        Kind::Spider if context.mode == Mode::One => (7, vec![6, 5, 4], true),
        Kind::Spider => (7, vec![19], false),
        Kind::Klondike => (33, vec![19, 5], true),
        Kind::Tripeaks => (33, vec![19, 44], false),
    };
    let height = 96 + (group.len() - 1) * 27;
    view! {<figure class="help-example">
        <div class="help-cards" aria-label="Пример хода">
            <div class="help-stack"><ExampleCard id=destination offset=0/></div>
            <span class="help-arrow" aria-hidden="true">{if context.kind==Kind::Tripeaks {"→"} else {"←"}}</span>
            <div class="help-stack" class:help-group=grouped style=format!("height:{height}px")>
                {group.into_iter().enumerate().map(|(offset,id)|view!{<ExampleCard id=id offset=offset/>}).collect_view()}
            </div>
        </div>
        <figcaption>{rules(context).example}</figcaption>
    </figure>}.into_any()
}
#[component]
pub fn Screen(context: Context, on_back: Callback<()>) -> impl IntoView {
    let content = rules(context);
    view! {
        <div class="help-heading">
            <button class="help-back" on:click=move |_|on_back.run(())>"‹ Назад"</button>
            <div><p>"Как играть"</p><h2 tabindex="-1">{context.title()}</h2></div>
        </div>
        <div class="help-body">
            <section><h3>"Цель"</h3><p>{content.goal}</p></section>
            <section><h3>"Как сделать ход"</h3><p>{content.action}</p></section>
            <Example context=context/>
            <section class="help-questions" aria-label="Вопросы о правилах">
                {content.questions.iter().map(|&(question,answer)|view!{<details><summary>{question}</summary><p>{answer}</p></details>}).collect_view()}
            </section>
        </div>
        <div class="help-return"><button class="primary" on:click=move |_|on_back.run(())>{context.return_label()}</button></div>
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use spider_engine::{
        Game, Move,
        klondike::{self, Command, Source, Target},
    };
    #[test]
    fn chosen_game_and_variant_override_table_only_from_new_game() {
        let c = Context::capture(
            Kind::Spider,
            Mode::Two,
            Kind::Klondike,
            Mode::Four,
            Dialog::ConfirmNew,
        );
        assert_eq!(
            (c.kind, c.return_to, c.return_label()),
            (Kind::Klondike, Dialog::ConfirmNew, "Назад к выбору")
        );
        let c = Context::capture(
            Kind::Spider,
            Mode::Two,
            Kind::Klondike,
            Mode::Four,
            Dialog::None,
        );
        assert_eq!(
            (c.kind, c.mode, c.return_to),
            (Kind::Spider, Mode::Two, Dialog::None)
        );
        assert!(rules(c).example.contains("разные масти"));
    }
    #[test]
    fn illustrated_examples_are_accepted_by_the_engines() {
        for mode in Mode::ALL {
            let mut g = Game::new_with_mode(42, mode);
            g.state.columns = std::array::from_fn(|_| vec![]);
            let c = |id| Card { id, open: true };
            g.state.columns[0] = if mode == Mode::One {
                vec![c(6), c(5), c(4)]
            } else {
                vec![c(19)]
            };
            g.state.columns[1] = vec![c(7)];
            assert!(g.legal(Move {
                from: 0,
                start: 0,
                to: 1
            }));
            g.apply(Move {
                from: 0,
                start: 0,
                to: 1,
            })
            .unwrap();
            if mode != Mode::One {
                assert!(!g.selectable(1, 0));
                assert!(g.selectable(1, 1));
            }
        }
        let mut k = klondike::Game::new(42);
        k.state.columns = std::array::from_fn(|_| vec![]);
        k.state.columns[0] = vec![Card { id: 19, open: true }, Card { id: 5, open: true }];
        k.state.columns[1] = vec![Card { id: 33, open: true }];
        assert!(k.legal(Command::Move {
            from: Source::Tableau {
                column: 0,
                start: 0
            },
            to: Target::Tableau(1)
        }));
        assert!(spider_engine::tripeaks::adjacent(8, 7));
        assert!(spider_engine::tripeaks::adjacent(7, 6));
    }
}
