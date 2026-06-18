use std::collections::HashMap;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};
use crate::db::types::ColumnSchema;

const MAX_COLS_CENTER: usize = 14;
const MAX_COLS_ADJ: usize = 8;
const MIN_GAP: usize = 6;

pub enum ErdGraphAction {
    None,
    Back,
}

pub struct ErdGraphScreen {
    pub center: String,
    pub all_schemas: HashMap<String, Vec<ColumnSchema>>,
    pub selected: usize, // 0=center, 1..=N_inc=incoming, N_inc+1..=end=outgoing
    outgoing: Vec<String>,
    incoming: Vec<String>,
}

impl ErdGraphScreen {
    pub fn new(center: String, all_schemas: HashMap<String, Vec<ColumnSchema>>) -> Self {
        let (outgoing, incoming) = Self::compute_connections(&center, &all_schemas);
        Self { center, all_schemas, selected: 0, outgoing, incoming }
    }

    fn navigate_to(&mut self, table: String) {
        let (outgoing, incoming) = Self::compute_connections(&table, &self.all_schemas);
        self.center  = table;
        self.outgoing = outgoing;
        self.incoming = incoming;
        self.selected = 0;
    }

    fn compute_connections(
        center: &str,
        schemas: &HashMap<String, Vec<ColumnSchema>>,
    ) -> (Vec<String>, Vec<String>) {
        let mut outgoing = vec![];
        let mut incoming = vec![];

        if let Some(cols) = schemas.get(center) {
            for col in cols {
                if let Some(ref fk) = col.fk
                    && !outgoing.contains(&fk.table) && fk.table != center {
                        outgoing.push(fk.table.clone());
                    }
            }
        }

        for (table, cols) in schemas {
            if table == center { continue; }
            let refs_center = cols.iter().any(|c| {
                c.fk.as_ref().is_some_and(|fk| fk.table == center)
            });
            if refs_center && !incoming.contains(table) {
                incoming.push(table.clone());
            }
        }

        incoming.sort();
        outgoing.sort();
        (outgoing, incoming)
    }

    fn total_adj(&self) -> usize {
        self.outgoing.len() + self.incoming.len()
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> ErdGraphAction {
        let n = self.total_adj() + 1;
        match key.code {
            KeyCode::Char('q') | KeyCode::Esc => ErdGraphAction::Back,
            KeyCode::Char('j') | KeyCode::Down | KeyCode::Tab => {
                self.selected = (self.selected + 1) % n;
                ErdGraphAction::None
            }
            KeyCode::Char('k') | KeyCode::Up | KeyCode::BackTab => {
                self.selected = (self.selected + n - 1) % n;
                ErdGraphAction::None
            }
            KeyCode::Enter => {
                if self.selected > 0 {
                    let table = if self.selected <= self.incoming.len() {
                        self.incoming[self.selected - 1].clone()
                    } else {
                        self.outgoing[self.selected - self.incoming.len() - 1].clone()
                    };
                    self.navigate_to(table);
                }
                ErdGraphAction::None
            }
            _ => ErdGraphAction::None,
        }
    }

    pub fn draw(f: &mut Frame<'_>, screen: &mut ErdGraphScreen, area: Rect) {
        let w = area.width as usize;
        let canvas_h = area.height.saturating_sub(1) as usize;

        // ── Layout ────────────────────────────────────────────────────────────

        let has_left  = !screen.incoming.is_empty();
        let has_right = !screen.outgoing.is_empty();

        let n_cols = match (has_left, has_right) {
            (true, true)   => 3,
            (true, false) | (false, true) => 2,
            (false, false) => 1,
        };

        let total_gap  = w.saturating_sub(n_cols * 20); // min 20 per box
        let gap        = (total_gap / (n_cols.max(1))).clamp(MIN_GAP, 16);
        let box_w      = ((w.saturating_sub(gap * (n_cols - 1))) / n_cols).max(20);

        let (left_x, center_x, right_x) = match (has_left, has_right) {
            (true,  true)  => (0usize, box_w + gap, 2 * (box_w + gap)),
            (true,  false) => (0,      box_w + gap, 0),
            (false, true)  => (0,      0,            box_w + gap),
            (false, false) => (0,      w.saturating_sub(box_w) / 2, 0),
        };

        // ── Center box ────────────────────────────────────────────────────────

        let empty: Vec<ColumnSchema> = vec![];
        let center_cols = screen.all_schemas.get(&screen.center).unwrap_or(&empty);
        let center_h = box_height(center_cols.len(), MAX_COLS_CENTER);
        let center_y = canvas_h.saturating_sub(center_h) / 2;

        // ── Left/right box heights and y positions ────────────────────────────

        let incoming_hs: Vec<usize> = screen.incoming.iter()
            .map(|t| box_height(screen.all_schemas.get(t).map_or(0, |c| c.len()), MAX_COLS_ADJ))
            .collect();
        let incoming_ys = distribute(&incoming_hs, canvas_h);

        let outgoing_hs: Vec<usize> = screen.outgoing.iter()
            .map(|t| box_height(screen.all_schemas.get(t).map_or(0, |c| c.len()), MAX_COLS_ADJ))
            .collect();
        let outgoing_ys = distribute(&outgoing_hs, canvas_h);

        // ── Build canvas ──────────────────────────────────────────────────────

        let mut cv = Canvas::new(w, canvas_h);

        // Center box
        let center_sel = screen.selected == 0;
        let center_border = if center_sel {
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::Yellow)
        };
        draw_box(&mut cv, center_x, center_y, box_w, &screen.center,
                 center_cols, MAX_COLS_CENTER, center_border);

        // Incoming boxes + arrows (left → center)
        for (i, table) in screen.incoming.iter().enumerate() {
            let by  = incoming_ys[i];
            let bh  = incoming_hs[i];
            let cols = screen.all_schemas.get(table).map(|v| v.as_slice()).unwrap_or(&[]);
            let sel  = screen.selected == i + 1;
            let bdr  = adj_style(sel);
            draw_box(&mut cv, left_x, by, box_w, table, cols, MAX_COLS_ADJ, bdr);

            // Arrow: right edge of left box → left edge of center box, at FK column row
            let src_x  = left_x + box_w;                         // one past right border
            let src_y  = by + bh / 2;
            let dst_x  = center_x;                               // center left border (arrow head here)
            // Find the column row in center box that this table references
            let dst_y  = center_cols.iter().take(MAX_COLS_CENTER).position(|c| {
                cols.iter().any(|lc| lc.fk.as_ref().is_some_and(|fk| fk.table == screen.center && fk.column == c.name))
            }).map(|p| center_y + 3 + p).unwrap_or(center_y + center_h / 2);

            draw_h_arrow(&mut cv, src_x, src_y, dst_x.saturating_sub(1), dst_y, Color::Cyan);
            cv.put(dst_x, dst_y, '►', Style::default().fg(Color::Cyan));
        }

        // Outgoing boxes + arrows (center → right)
        for (i, table) in screen.outgoing.iter().enumerate() {
            let by   = outgoing_ys[i];
            let bh   = outgoing_hs[i];
            let cols = screen.all_schemas.get(table).map(|v| v.as_slice()).unwrap_or(&[]);
            let adj_i = i + screen.incoming.len();
            let sel   = screen.selected == adj_i + 1;
            let bdr   = adj_style(sel);
            draw_box(&mut cv, right_x, by, box_w, table, cols, MAX_COLS_ADJ, bdr);

            // Arrow: center right edge → right box left edge, from FK column row
            let src_x = center_x + box_w;
            let src_y = center_cols.iter().take(MAX_COLS_CENTER).position(|c| {
                c.fk.as_ref().is_some_and(|fk| fk.table == *table)
            }).map(|p| center_y + 3 + p).unwrap_or(center_y + center_h / 2);
            let dst_x = right_x;
            let dst_y = by + bh / 2;

            draw_h_arrow(&mut cv, src_x, src_y, dst_x.saturating_sub(1), dst_y, Color::Green);
            cv.put(dst_x, dst_y, '►', Style::default().fg(Color::Green));
        }

        // ── Render ────────────────────────────────────────────────────────────

        let lines = cv.into_lines();
        f.render_widget(
            Paragraph::new(lines),
            Rect { x: area.x, y: area.y, width: area.width, height: canvas_h as u16 },
        );

        // Help bar (1 row, no border)
        let sel_name = selected_name(screen);
        let help = format!(
            " j/k: navigate  Enter: focus [{}]  q: back to table list ",
            sel_name
        );
        f.render_widget(
            Paragraph::new(help).style(Style::default().fg(Color::Gray).bg(Color::Reset)),
            Rect { x: area.x, y: area.y + canvas_h as u16, width: area.width, height: 1 },
        );
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn selected_name(screen: &ErdGraphScreen) -> &str {
    if screen.selected == 0 {
        &screen.center
    } else if screen.selected <= screen.incoming.len() {
        &screen.incoming[screen.selected - 1]
    } else {
        let i = screen.selected - screen.incoming.len() - 1;
        screen.outgoing.get(i).map(|s| s.as_str()).unwrap_or(&screen.center)
    }
}

fn box_height(col_count: usize, max_cols: usize) -> usize {
    let shown    = col_count.min(max_cols);
    let has_more = col_count > max_cols;
    4 + shown + usize::from(has_more)
}

fn adj_style(selected: bool) -> Style {
    if selected {
        Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::Gray)
    }
}

/// Distribute N boxes of given heights evenly across `available` rows.
fn distribute(heights: &[usize], available: usize) -> Vec<usize> {
    if heights.is_empty() { return vec![]; }
    let n = heights.len();
    let total: usize = heights.iter().sum();
    if total >= available {
        // Tight packing
        let mut ys = vec![];
        let mut y  = 0usize;
        for &h in heights {
            ys.push(y);
            y += h + 1;
        }
        return ys;
    }
    let gap = (available - total) / (n + 1);
    let gap = gap.max(1);
    let mut ys = vec![];
    let mut y  = gap;
    for &h in heights {
        ys.push(y);
        y += h + gap;
    }
    ys
}

fn trunc(s: &str, max: usize) -> String {
    let chars: Vec<char> = s.chars().collect();
    if chars.len() <= max { return s.to_string(); }
    if max == 0 { return String::new(); }
    let t: String = chars.iter().take(max.saturating_sub(1)).collect();
    format!("{}…", t)
}

// ── Canvas ────────────────────────────────────────────────────────────────────

struct Canvas {
    w: usize,
    h: usize,
    cells: Vec<(char, Style)>,
}

impl Canvas {
    fn new(w: usize, h: usize) -> Self {
        Self { w, h, cells: vec![(' ', Style::default()); w * h] }
    }

    fn put(&mut self, x: usize, y: usize, c: char, s: Style) {
        if x < self.w && y < self.h {
            self.cells[y * self.w + x] = (c, s);
        }
    }

    fn put_str(&mut self, x: usize, y: usize, text: &str, s: Style) {
        for (i, c) in text.chars().enumerate() {
            self.put(x + i, y, c, s);
        }
    }

    fn hline(&mut self, x1: usize, x2: usize, y: usize, c: char, s: Style) {
        for x in x1..=x2 { self.put(x, y, c, s); }
    }

    fn vline(&mut self, x: usize, y1: usize, y2: usize, s: Style) {
        let (a, b) = if y1 <= y2 { (y1, y2) } else { (y2, y1) };
        for y in a..=b { self.put(x, y, '│', s); }
    }

    fn draw_border(&mut self, x: usize, y: usize, w: usize, h: usize, s: Style) {
        if w < 2 || h < 2 { return; }
        self.put(x, y, '┌', s);
        self.hline(x + 1, x + w - 2, y, '─', s);
        self.put(x + w - 1, y, '┐', s);
        self.put(x, y + h - 1, '└', s);
        self.hline(x + 1, x + w - 2, y + h - 1, '─', s);
        self.put(x + w - 1, y + h - 1, '┘', s);
        for row in 1..h - 1 {
            self.put(x, y + row, '│', s);
            self.put(x + w - 1, y + row, '│', s);
        }
    }

    fn separator(&mut self, x: usize, y: usize, w: usize, s: Style) {
        self.put(x, y, '├', s);
        self.hline(x + 1, x + w - 2, y, '─', s);
        self.put(x + w - 1, y, '┤', s);
    }

    fn into_lines(self) -> Vec<Line<'static>> {
        (0..self.h).map(|y| {
            let mut spans: Vec<Span<'static>> = vec![];
            let mut cur_s  = Style::default();
            let mut cur_t  = String::new();
            for x in 0..self.w {
                let (c, s) = self.cells[y * self.w + x];
                if s == cur_s {
                    cur_t.push(c);
                } else {
                    if !cur_t.is_empty() {
                        spans.push(Span::styled(std::mem::take(&mut cur_t), cur_s));
                    }
                    cur_s = s;
                    cur_t.push(c);
                }
            }
            if !cur_t.is_empty() {
                spans.push(Span::styled(cur_t, cur_s));
            }
            Line::from(spans)
        }).collect()
    }
}

// ── Box and arrow drawing ─────────────────────────────────────────────────────

#[allow(clippy::too_many_arguments)]
fn draw_box(
    cv: &mut Canvas,
    x: usize,
    y: usize,
    w: usize,
    name: &str,
    cols: &[ColumnSchema],
    max_cols: usize,
    border: Style,
) {
    let inner   = w.saturating_sub(2);
    let shown   = cols.len().min(max_cols);
    let has_more = cols.len() > max_cols;
    let h       = 4 + shown + usize::from(has_more);

    cv.draw_border(x, y, w, h, border);

    // Title
    let title_s = border.add_modifier(Modifier::BOLD);
    cv.put_str(x + 1, y + 1, &format!("{:<width$}", trunc(name, inner), width = inner), title_s);

    // Separator
    cv.separator(x, y + 2, w, border);

    // Columns
    for (i, col) in cols.iter().take(max_cols).enumerate() {
        let row = y + 3 + i;
        let (badge, badge_s) = if col.is_pk {
            ("[PK]", Style::default().fg(Color::Yellow))
        } else if col.fk.is_some() {
            ("[FK]", Style::default().fg(Color::Magenta))
        } else {
            ("    ", Style::default().fg(Color::Gray))
        };
        cv.put_str(x + 1, row, badge, badge_s);
        cv.put(x + 5, row, ' ', Style::default());

        let type_raw  = trunc(&col.type_name.to_lowercase(), 10);
        let type_w    = type_raw.chars().count();
        let name_w    = inner.saturating_sub(6 + type_w + 1);
        cv.put_str(x + 6,                  row, &trunc(&col.name, name_w), Style::default().fg(Color::White));
        cv.put_str(x + w - 1 - type_w, row, &type_raw,                    Style::default().fg(Color::Gray));
    }

    if has_more {
        let more = format!("+{} more", cols.len() - max_cols);
        cv.put_str(x + 1, y + 3 + shown, &trunc(&more, inner), Style::default().fg(Color::Gray));
    }
}

/// Draw a right-pointing arrow from (x1, y1) to (x2, y2), not including the
/// arrow head character — caller places '►' at x2+1 / the box border.
fn draw_h_arrow(cv: &mut Canvas, x1: usize, y1: usize, x2: usize, y2: usize, color: Color) {
    if x1 > x2 { return; }
    let s = Style::default().fg(color);

    if y1 == y2 {
        cv.hline(x1, x2, y1, '─', s);
        return;
    }

    let x_mid = (x1 + x2) / 2;

    // First horizontal segment on row y1
    if x1 < x_mid { cv.hline(x1, x_mid - 1, y1, '─', s); }

    // Corner at elbow (top)
    cv.put(x_mid, y1, if y2 > y1 { '┐' } else { '┘' }, s);

    // Vertical segment
    if y2 > y1 {
        if y1 + 1 < y2 { cv.vline(x_mid, y1 + 1, y2 - 1, s); }
        cv.put(x_mid, y2, '└', s);
    } else {
        if y2 + 1 < y1 { cv.vline(x_mid, y2 + 1, y1 - 1, s); }
        cv.put(x_mid, y2, '┌', s);
    }

    // Second horizontal segment on row y2
    if x_mid < x2 { cv.hline(x_mid + 1, x2, y2, '─', s); }
}
