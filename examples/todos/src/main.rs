//! A to-do list on the TAIM view API: a list that follows state
//! (`each`), a section that appears and disappears (`when`), and
//! background work (`spawn`).
// No console window behind the app in Windows release builds.
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]
use std::time::Duration;
use taim::prelude::*;

#[derive(Debug, Clone, PartialEq)]
struct Todo {
    id: u32,
    title: String,
    done: bool,
}

fn main() -> taim::Result {
    taim::launch("Todos", |cx| {
        let todos = cx.signal(Vec::<Todo>::new());
        let draft = cx.signal(String::new());
        let next_id = cx.signal(1_u32);
        let loading = cx.signal(false);

        let add = move || {
            let title = draft.get().trim().to_owned();
            if title.is_empty() {
                return;
            }
            let id = next_id.update(|id| std::mem::replace(id, *id + 1));
            todos.update(|todos| {
                todos.push(Todo {
                    id,
                    title,
                    done: false,
                })
            });
            draft.set(String::new());
        };

        let sample = move || {
            loading.set(true);
            spawn(async move {
                std::thread::sleep(Duration::from_millis(600));
                todos.update(|todos| {
                    for title in ["Read the TAIM API doc", "Port a widget", "Ship it"] {
                        todos.push(Todo {
                            id: 1000 + todos.len() as u32,
                            title: title.into(),
                            done: false,
                        });
                    }
                });
                loading.set(false);
            });
        };

        container(
            column(views![
                text("Todos").size(28),
                row(views![
                    text_field("What needs doing?", draft).on_submit(add),
                    button("Add")
                        .on_press(add)
                        .enabled(move || !draft.get().trim().is_empty()),
                ])
                .gap(8.0),
                each(todos, move |todo: &Todo| {
                    let id = todo.id;
                    row(views![
                        text(if todo.done {
                            format!("✓ {}", todo.title)
                        } else {
                            todo.title.clone()
                        }),
                        space(),
                        button(if todo.done { "Undo" } else { "Done" }).on_press(move || {
                            todos.update(|todos| {
                                if let Some(todo) = todos.iter_mut().find(|todo| todo.id == id) {
                                    todo.done = !todo.done;
                                }
                            });
                        }),
                        button("Delete").on_press(move || {
                            todos.update(|todos| todos.retain(|todo| todo.id != id));
                        }),
                    ])
                    .gap(8.0)
                    .align(Align::Center)
                    .fill_width()
                })
                .gap(6.0),
                when(move || todos.get().is_empty(), || text("Nothing to do.")),
                text(move || {
                    let todos = todos.get();
                    let open = todos.iter().filter(|todo| !todo.done).count();
                    format!("{open} open, {} done", todos.len() - open)
                }),
                button("Load samples")
                    .on_press(sample)
                    .enabled(move || !loading.get()),
                when(loading, || text("Loading…")),
            ])
            .gap(12.0)
            .padding(24.0)
            .fill_width(),
        )
        .fill()
    })
}
