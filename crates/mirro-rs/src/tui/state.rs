#[cfg(feature = "archlinux")]
use archlinux::{
    ArchLinux, Country, {DateTime, Protocol, Utc},
};

use itertools::Itertools;
use log::{debug, error, info, warn};
use tui::{
    style::{Color, Modifier, Style},
    widgets::{Cell, Row},
};
use unicode_width::UnicodeWidthStr;

use crate::tui::actions::Action;

use super::{
    actions::Actions,
    dispatch::{filter::Filter, sort::ViewSort},
    inputs::key::Key,
    io::IoEvent,
};

#[derive(Debug, PartialEq, Eq)]
pub enum AppReturn {
    Exit,
    Continue,
}

pub struct App {
    pub show_popup: bool,
    pub actions: Actions,
    #[cfg(feature = "archlinux")]
    pub mirrors: Option<ArchLinux>,
    pub io_tx: tokio::sync::mpsc::Sender<IoEvent>,
    pub input: String,
    pub input_cursor_position: usize,
    pub show_input: bool,
    pub active_sort: Vec<ViewSort>,
    pub active_filter: Vec<Filter>,
    pub scroll_pos: isize,
    pub filtered_countries: Vec<(Country, usize)>,
    pub selected_mirrors: Vec<SelectedMirror>,
    pub table_viewport_height: u16,
}

#[derive(Debug, Clone)]
pub struct SelectedMirror {
    pub country_code: String,
    pub protocol: Protocol,
    pub completion_pct: f32,
    pub delay: Option<i64>,
    pub duration_avg: Option<f64>,
    pub duration_stddev: Option<f64>,
    pub last_sync: Option<DateTime<Utc>>,
}

impl App {
    #[cfg(feature = "archlinux")]
    pub fn new(io_tx: tokio::sync::mpsc::Sender<IoEvent>) -> Self {
        Self {
            actions: vec![Action::Quit].into(),
            show_popup: true,
            show_input: false,
            mirrors: None,
            io_tx,
            input: String::default(),
            input_cursor_position: 0,
            active_sort: vec![ViewSort::Alphabetical],
            active_filter: vec![Filter::Https, Filter::Http],
            scroll_pos: 0,
            table_viewport_height: 0,
            selected_mirrors: vec![],
            filtered_countries: vec![],
        }
    }

    pub async fn dispatch_action(&mut self, key: Key) -> AppReturn {
        if let Some(action) = self.actions.find(key) {
            if key.is_exit() && !self.show_input {
                AppReturn::Exit
            } else if self.show_input {
                match action {
                    Action::Quit => {
                        if key == Key::Char('q') {
                            insert_character(self, 'q');
                        }
                    }
                    Action::NavigateUp => {
                        if key == Key::Char('k') {
                            insert_character(self, 'k');
                        }
                    }
                    Action::NavigateDown => {
                        if key == Key::Char('j') {
                            insert_character(self, 'j');
                        }
                    }
                    Action::ViewSortAlphabetically => insert_character(self, '1'),
                    Action::ViewSortMirrorCount => insert_character(self, '2'),
                    _ => {}
                }
                AppReturn::Continue
            } else {
                match action {
                    Action::ClosePopUp => {
                        self.show_popup = !self.show_popup;
                        AppReturn::Continue
                    }
                    Action::Quit => AppReturn::Continue,
                    Action::ShowInput => {
                        self.show_input = !self.show_input;
                        AppReturn::Continue
                    }
                    Action::NavigateUp => {
                        self.previous();
                        AppReturn::Continue
                    }
                    Action::NavigateDown => {
                        self.next();
                        AppReturn::Continue
                    }
                    Action::FilterHttps => insert_filter(self, Filter::Https),
                    Action::FilterHttp => insert_filter(self, Filter::Http),
                    Action::FilterRsync => insert_filter(self, Filter::Rsync),
                    Action::FilterSyncing => insert_filter(self, Filter::InSync),
                    Action::ViewSortAlphabetically => insert_sort(self, ViewSort::Alphabetical),
                    Action::ViewSortMirrorCount => insert_sort(self, ViewSort::MirrorCount),
                    Action::ToggleSelect => {
                        self.focused_country();
                        AppReturn::Continue
                    }
                }
            }
        } else {
            if self.show_input {
                match key {
                    Key::Enter => todo!(),
                    Key::Backspace => {
                        if !self.input.is_empty() {
                            self.input = format!(
                                "{}{}",
                                &self.input[..self.input_cursor_position - 1],
                                &self.input[self.input_cursor_position..]
                            );
                            self.input_cursor_position -= 1;
                        }
                    }
                    Key::Left => {
                        if self.input_cursor_position > 0 {
                            self.input_cursor_position -= 1;
                        }
                    }
                    Key::Right => {
                        if self.input_cursor_position < self.input.width() {
                            self.input_cursor_position += 1;
                        } else {
                            self.input_cursor_position = self.input.width();
                        };
                    }
                    Key::Delete => {
                        if self.input_cursor_position < self.input.width() {
                            self.input.remove(self.input_cursor_position);
                        }
                    }
                    Key::Home => {
                        self.input_cursor_position = 0;
                    }
                    Key::End => {
                        self.input_cursor_position = self.input.width();
                    }
                    Key::Char(c) => {
                        insert_character(self, c);
                        self.scroll_pos = 0;
                    }
                    Key::Esc => {
                        self.show_input = false;
                    }
                    _ => {
                        warn!("No action associated to {key}");
                    }
                }
            } else {
                warn!("No action associated to {key}");
            }
            AppReturn::Continue
        }
    }

    pub async fn dispatch(&mut self, action: IoEvent) {
        self.show_popup = true;
        if let Err(e) = self.io_tx.send(action).await {
            self.show_popup = false;
            error!("Error from dispatch {e}");
        };
    }

    pub async fn update_on_tick(&mut self) -> AppReturn {
        AppReturn::Continue
    }

    pub fn ready(&mut self) {
        self.actions = vec![
            Action::ShowInput,
            Action::ClosePopUp,
            Action::Quit,
            Action::NavigateDown,
            Action::NavigateUp,
            Action::FilterHttp,
            Action::FilterHttps,
            Action::FilterRsync,
            Action::FilterSyncing,
            Action::ViewSortAlphabetically,
            Action::ViewSortMirrorCount,
            Action::ToggleSelect,
        ]
        .into();
        self.show_popup = false;
    }

    pub fn next(&mut self) {
        if self.scroll_pos + 1 == self.filtered_countries.len() as isize {
            self.scroll_pos = 0;
        } else {
            self.scroll_pos += 1;
        }
    }

    pub fn previous(&mut self) {
        if self.scroll_pos - 1 < 0 {
            self.scroll_pos = (self.filtered_countries.len() - 1) as isize;
        } else {
            self.scroll_pos -= 1;
        }
    }

    pub fn view_fragments<'a, T>(&'a self, iter: &'a [T]) -> Vec<&'a [T]> {
        iter.chunks(self.table_viewport_height.into()).collect_vec()
    }

    pub fn rows(&self) -> Vec<Row> {
        self.filtered_countries
            .iter()
            .enumerate()
            .map(|(idx, (f, count))| {
                let mut selected = false;
                let default = format!("├─ [{}] {}", f.code, f.name);
                let item_name = match self.scroll_pos as usize == idx {
                    true => {
                        if idx == self.scroll_pos as usize {
                            selected = true;
                            format!("├─»[{}] {}«", f.code, f.name)
                        } else {
                            default
                        }
                    }
                    false => default,
                };

                let index = format!("  {idx}│");

                return Row::new([index, item_name, count.to_string()].iter().map(|c| {
                    Cell::from(c.clone()).style(if selected {
                        Style::default()
                            .add_modifier(Modifier::BOLD)
                            .fg(Color::Green)
                    } else {
                        Style::default().fg(Color::Gray)
                    })
                }));
            })
            .collect_vec()
    }

    pub fn view<T: Copy>(&self, fragment: &[T]) -> T {
        fragment[self.fragment_number()]
    }

    pub fn focused_country(&mut self) {
        if let Some(_items) = self.mirrors.as_ref() {
            let country = if self.scroll_pos < self.table_viewport_height as isize {
                let (country, _) = &self.filtered_countries[self.scroll_pos as usize];
                // we can directly index
                info!("selected: {}", country.name);
                country
            } else {
                let page = self.fragment_number();
                let index = (self.scroll_pos
                    - (page * self.table_viewport_height as usize) as isize)
                    as usize;
                let fragments = self.view_fragments(&self.filtered_countries);
                let frag = fragments[page];
                let (country, _) = &frag[index];
                info!("selected: {}", country.name);
                country
            };

            let mut mirrors = country
                .mirrors
                .iter()
                //     .filter(|f| {
                //         if self.in_sync_only() {
                //             if let Some(mirror_sync) = f.last_sync {
                //                 let duration = _items.last_check - mirror_sync;
                //                 duration.num_hours() <= 24
                //                     && self.active_filter.contains(&protocol_mapper(f.protocol))
                //             } else {
                //                 false
                //             }
                //         } else {
                //             self.active_filter.contains(&protocol_mapper(f.protocol))
                //         }
                //     })
                .map(|f| SelectedMirror {
                    country_code: country.code.to_string(),
                    protocol: f.protocol,
                    completion_pct: f.completion_pct,
                    delay: f.delay,
                    duration_avg: f.duration_avg,
                    duration_stddev: f.duration_stddev,
                    last_sync: f.last_sync,
                })
                .collect_vec();

            let pos = self
                .selected_mirrors
                .iter()
                .positions(|f| f.country_code == country.code)
                .collect_vec();

            if pos.is_empty() {
                self.selected_mirrors.append(&mut mirrors)
            } else {
                let new_items = self
                    .selected_mirrors
                    .iter()
                    .filter_map(|f| {
                        if f.country_code != country.code {
                            Some(f.clone())
                        } else {
                            None
                        }
                    })
                    .collect_vec();

                self.selected_mirrors = new_items;
            }
        }
    }

    fn fragment_number(&self) -> usize {
        (self.scroll_pos / self.table_viewport_height as isize) as usize
    }

    // pub fn in_sync_only(&self) -> bool {
    //     self.active_filter.contains(&Filter::InSync)
    // }
}

fn insert_character(app: &mut App, key: char) {
    app.input.insert(app.input_cursor_position, key);
    app.input_cursor_position += 1;
    app.scroll_pos = 0;
}

fn insert_filter(app: &mut App, filter: Filter) -> AppReturn {
    if let Some(idx) = app.active_filter.iter().position(|f| *f == filter) {
        debug!("protocol filter: removed {filter}");
        app.active_filter.remove(idx);
    } else {
        debug!("protocol filter: added {filter}");
        app.active_filter.push(filter);
    }
    app.scroll_pos = 0;
    AppReturn::Continue
}

fn insert_sort(app: &mut App, view: ViewSort) -> AppReturn {
    app.active_sort.clear();
    app.active_sort.push(view);
    AppReturn::Continue
}
