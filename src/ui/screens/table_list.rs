use std::collections::HashMap;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap},
    Frame,
};
use crate::db::types::{ColumnSchema, TableKind, TableObject};

pub enum TableListAction {
    None,
    OpenTable { name: String, is_view: bool },
    OpenEditor,
    OpenErd(String),
    Disconnect,
    SelectionChanged,
}

pub struct TableListScreen {
    pub tables: Vec<TableObject>,
    pub list_state: ListState,
    pub filter: String,
    pub filter_mode: bool,
    pub status: Option<String>,
    pub db_info: String,
    pub is_kv: bool,
    pub all_schemas: HashMap<String, Vec<ColumnSchema>>,
    pub schemas_loading: bool,
}

impl TableListScreen {
    pub fn new() -> Self {
        Self {
            tables: vec![],
            list_state: ListState::default(),
            filter: String::new(),
            filter_mode: false,
            status: Some("Loading…".into()),
            db_info: String::new(),
            is_kv: false,
            all_schemas: HashMap::new(),
            schemas_loading: false,
        }
    }

    pub fn set_tables(&mut self, tables: Vec<TableObject>) {
        self.tables = tables;
        self.status = None;
        self.schemas_loading = true;
        if !self.tables.is_empty() {
            self.list_state.select(Some(0));
        }
    }

    pub fn set_tables_kv(&mut self, names: Vec<String>) {
        self.is_kv = true;
        let mut sorted = names;
        sorted.sort();
        self.tables = sorted.into_iter()
            .map(|name| TableObject { name, kind: TableKind::Table })
            .collect();
        self.status = None;
        if !self.tables.is_empty() {
            self.list_state.select(Some(0));
        }
    }

    pub fn set_all_schemas(&mut self, schemas: HashMap<String, Vec<ColumnSchema>>) {
        self.all_schemas = schemas;
        self.schemas_loading = false;
    }

    pub fn set_error(&mut self, msg: String) {
        self.status = Some(msg);
    }

    fn filtered(&self) -> Vec<&TableObject> {
        if self.filter.is_empty() {
            self.tables.iter().collect()
        } else {
            let f = self.filter.to_lowercase();
            self.tables.iter().filter(|t| t.name.to_lowercase().contains(&f)).collect()
        }
    }

    fn selected_object(&self) -> Option<(String, bool)> {
        self.list_state
            .selected()
            .and_then(|i| self.filtered().get(i).map(|obj| {
                (obj.name.clone(), obj.kind == TableKind::View)
            }))
    }

    pub fn selected_table_name(&self) -> Option<String> {
        self.selected_object().map(|(name, _)| name)
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> TableListAction {
        if self.filter_mode {
            return self.handle_filter(key);
        }
        match key.code {
            KeyCode::Char('q') | KeyCode::Esc => TableListAction::Disconnect,
            KeyCode::Char('j') | KeyCode::Down  => {
                self.select_next();
                self.emit_selection_changed()
            }
            KeyCode::Char('k') | KeyCode::Up    => {
                self.select_prev();
                self.emit_selection_changed()
            }
            KeyCode::Char('/')                   => { self.filter_mode = true; TableListAction::None }
            KeyCode::Char('e')                   => TableListAction::OpenEditor,
            KeyCode::Char('r') => {
                if let Some(name) = self.selected_table_name() {
                    TableListAction::OpenErd(name)
                } else {
                    TableListAction::None
                }
            }
            KeyCode::Enter => {
                if let Some((name, is_view)) = self.selected_object() {
                    TableListAction::OpenTable { name, is_view }
                } else {
                    TableListAction::None
                }
            }
            _ => TableListAction::None,
        }
    }

    fn emit_selection_changed(&self) -> TableListAction {
        if self.selected_table_name().is_some() {
            TableListAction::SelectionChanged
        } else {
            TableListAction::None
        }
    }

    fn handle_filter(&mut self, key: KeyEvent) -> TableListAction {
        match key.code {
            KeyCode::Esc => {
                self.filter_mode = false;
                self.filter.clear();
                self.reset_selection();
            }
            KeyCode::Enter => {
                self.filter_mode = false;
            }
            KeyCode::Backspace => {
                self.filter.pop();
                self.reset_selection();
            }
            KeyCode::Char(c) => {
                self.filter.push(c);
                self.reset_selection();
            }
            _ => {}
        }
        TableListAction::None
    }

    fn reset_selection(&mut self) {
        let sel = if self.filtered().is_empty() { None } else { Some(0) };
        self.list_state.select(sel);
    }

    fn select_next(&mut self) {
        let len = self.filtered().len();
        if len == 0 { return; }
        let next = self.list_state.selected().map_or(0, |i| (i + 1).min(len - 1));
        self.list_state.select(Some(next));
    }

    fn select_prev(&mut self) {
        let len = self.filtered().len();
        if len == 0 { return; }
        let prev = self.list_state.selected().map_or(0, |i| i.saturating_sub(1));
        self.list_state.select(Some(prev));
    }

    pub fn draw(f: &mut Frame<'_>, screen: &mut TableListScreen, area: Rect) {
        let vert = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(0), Constraint::Length(3)])
            .split(area);
        let main_area = vert[0];
        let help_area = vert[1];

        if screen.is_kv {
            Self::draw_table_list(f, screen, main_area);
        } else {
            let horiz = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Length(28), Constraint::Min(0)])
                .split(main_area);
            Self::draw_table_list(f, screen, horiz[0]);
            Self::draw_schema_panel(f, screen, horiz[1]);
        }

        let help_text = if screen.filter_mode {
            let filter_display = format!("/{}", screen.filter);
            f.set_cursor(help_area.x + 1 + filter_display.len() as u16, help_area.y + 1);
            filter_display
        } else if screen.is_kv {
            " j/k: move   Enter: open   e: SQL editor   r: ERD   /: filter   Ctrl+T: new tab   Ctrl+W: close tab   q: disconnect ".into()
        } else {
            " j/k: move   Enter: open   e: SQL editor   r: ERD   /: filter   Ctrl+T: new tab   Ctrl+W: close tab   q: disconnect ".into()
        };

        let help_style = if screen.filter_mode {
            Style::default().fg(Color::Yellow)
        } else {
            Style::default().fg(Color::Gray)
        };

        f.render_widget(
            Paragraph::new(help_text)
                .block(Block::default().borders(Borders::ALL))
                .style(help_style),
            help_area,
        );
    }

    fn draw_table_list(f: &mut Frame<'_>, screen: &mut TableListScreen, area: Rect) {
        let (filtered_items, total): (Vec<(String, bool)>, usize) = {
            let v = screen.filtered();
            let items = v.iter().map(|o| (o.name.clone(), o.kind == TableKind::View)).collect();
            (items, screen.tables.len())
        };

        let title = if screen.filter.is_empty() {
            format!(" Tables ({}) ", filtered_items.len())
        } else {
            format!(" Tables ({}/{}) ", filtered_items.len(), total)
        };

        let items: Vec<ListItem> = if let Some(ref msg) = screen.status {
            vec![ListItem::new(msg.as_str()).style(Style::default().fg(Color::Gray))]
        } else if filtered_items.is_empty() {
            vec![ListItem::new("No match").style(Style::default().fg(Color::Gray))]
        } else if screen.is_kv {
            filtered_items.iter().map(|(name, _)| ListItem::new(name.clone())).collect()
        } else {
            filtered_items.iter().map(|(name, is_view)| {
                if *is_view {
                    ListItem::new(Line::from(vec![
                        Span::styled("[V] ", Style::default().fg(Color::Cyan)),
                        Span::raw(name.clone()),
                    ]))
                } else {
                    ListItem::new(Line::from(vec![
                        Span::styled("[T] ", Style::default().fg(Color::Gray)),
                        Span::raw(name.clone()),
                    ]))
                }
            }).collect()
        };

        let list = List::new(items)
            .block(Block::default().title(title).borders(Borders::ALL))
            .highlight_style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))
            .highlight_symbol("> ");

        f.render_stateful_widget(list, area, &mut screen.list_state);
    }

    fn draw_schema_panel(f: &mut Frame<'_>, screen: &TableListScreen, area: Rect) {
        let Some(table_name) = screen.selected_table_name() else {
            f.render_widget(
                Paragraph::new("Select a table")
                    .block(Block::default().title(" Schema ").borders(Borders::ALL))
                    .style(Style::default().fg(Color::Gray)),
                area,
            );
            return;
        };

        if screen.schemas_loading {
            f.render_widget(
                Paragraph::new("Loading schema…")
                    .block(Block::default().title(format!(" {} ", table_name)).borders(Borders::ALL))
                    .style(Style::default().fg(Color::Gray)),
                area,
            );
            return;
        }

        let cols = match screen.all_schemas.get(&table_name) {
            Some(c) => c.clone(),
            None => {
                f.render_widget(
                    Paragraph::new("Schema not available")
                        .block(Block::default().title(format!(" {} ", table_name)).borders(Borders::ALL))
                        .style(Style::default().fg(Color::Gray)),
                    area,
                );
                return;
            }
        };

        let mut lines: Vec<Line> = vec![
            Line::from(""),
            Line::from(Span::styled(
                "  Columns",
                Style::default().fg(Color::Cyan).add_modifier(Modifier::UNDERLINED),
            )),
            Line::from(""),
        ];

        for col in &cols {
            let badge = if col.is_pk {
                Span::styled("[PK] ", Style::default().fg(Color::Yellow))
            } else if col.fk.is_some() {
                Span::styled("[FK] ", Style::default().fg(Color::Magenta))
            } else {
                Span::raw("     ")
            };

            let name_span = Span::styled(
                format!("{:<20}", col.name),
                Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
            );
            let type_span = Span::styled(
                col.type_name.to_lowercase(),
                Style::default().fg(Color::Gray),
            );

            let mut row_spans = vec![Span::raw("  "), badge, name_span, type_span];

            if let Some(ref fk) = col.fk {
                row_spans.push(Span::styled(
                    format!("  →{}.{}", fk.table, fk.column),
                    Style::default().fg(Color::Magenta),
                ));
            }

            lines.push(Line::from(row_spans));
        }

        // Outgoing FK relations
        let outgoing: Vec<_> = cols.iter()
            .filter_map(|c| c.fk.as_ref().map(|fk| (c.name.as_str(), fk)))
            .collect();

        if !outgoing.is_empty() {
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                "  Outgoing FK",
                Style::default().fg(Color::Cyan).add_modifier(Modifier::UNDERLINED),
            )));
            lines.push(Line::from(""));
            for (col_name, fk) in &outgoing {
                lines.push(Line::from(vec![
                    Span::raw("  "),
                    Span::styled(col_name.to_string(), Style::default().fg(Color::White)),
                    Span::styled("  ──►  ", Style::default().fg(Color::Gray)),
                    Span::styled(
                        format!("{}.{}", fk.table, fk.column),
                        Style::default().fg(Color::Magenta),
                    ),
                ]));
            }
        }

        // Incoming FK relations (other tables referencing this one)
        let incoming: Vec<(String, String, String)> = screen.all_schemas.iter()
            .filter(|(t, _)| **t != table_name)
            .flat_map(|(t, tcols)| {
                tcols.iter()
                    .filter_map(|c| c.fk.as_ref().and_then(|fk| {
                        if fk.table == table_name {
                            Some((t.clone(), c.name.clone(), fk.column.clone()))
                        } else {
                            None
                        }
                    }))
                    .collect::<Vec<_>>()
            })
            .collect();

        if !incoming.is_empty() {
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                "  Incoming FK",
                Style::default().fg(Color::Cyan).add_modifier(Modifier::UNDERLINED),
            )));
            lines.push(Line::from(""));
            for (from_table, from_col, to_col) in &incoming {
                lines.push(Line::from(vec![
                    Span::raw("  "),
                    Span::styled(
                        format!("{}.{}", from_table, from_col),
                        Style::default().fg(Color::Magenta),
                    ),
                    Span::styled("  ──►  ", Style::default().fg(Color::Gray)),
                    Span::styled(
                        format!("{}.{}", table_name, to_col),
                        Style::default().fg(Color::White),
                    ),
                ]));
            }
        }

        f.render_widget(
            Paragraph::new(lines)
                .block(Block::default().title(format!(" {} ", table_name)).borders(Borders::ALL))
                .wrap(Wrap { trim: false }),
            area,
        );
    }
}
