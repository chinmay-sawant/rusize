use anyhow::Result;
use colored::Colorize;
use crossterm::{
    cursor::{Hide, Show},
    event::{
        self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind, MouseButton,
        MouseEvent, MouseEventKind,
    },
    execute,
    terminal::{self, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Gauge, Paragraph, Row, Table, Wrap},
    Frame,
};
use std::collections::HashSet;
use std::io;
use std::sync::mpsc::{Receiver, Sender};
use std::time::Duration;

use crate::chars::Chars;
use crate::scanner::{self, DirNode};

pub struct ScanTarget {
    pub root_display: String,
    pub nodes: Vec<DirNode>,
    pub total_size: u64,
}

pub struct FullscreenGuard;

#[derive(Clone, Copy, Default)]
struct LayoutCache {
    drawers_area: Rect,
    details_area: Rect,
    help_area: Rect,
    divider_x: u16,
    drawer_rows: usize,
}

struct App {
    targets: Vec<ScanTarget>,
    expanded: HashSet<String>,
    selected: usize,
    should_quit: bool,
    min_bytes: u64,
    pane_ratio: u16,
    drawer_scroll: usize,
    show_help: bool,
    status_message: String,
    resizing: bool,
    layout: LayoutCache,
    tx: Sender<scanner::ScanResult>,
}

#[derive(Clone)]
struct VisibleItem {
    key: String,
    title: String,
    depth: usize,
    expandable: bool,
    expanded: bool,
    is_loading: bool,
    kind: ItemKind,
}

#[derive(Clone)]
enum ItemKind {
    Target(usize),
    Node { target_idx: usize, node_path: Vec<usize> },
}

struct DetailData {
    title: String,
    kind_label: &'static str,
    path: String,
    size: u64,
    root_label: String,
    root_share: f64,
    total_share: f64,
    direct_children: usize,
    visible_children: usize,
    descendants: usize,
    tracked_depth: usize,
    largest_child: Option<(String, u64)>,
    breakdown: Vec<(String, f64, u64)>,
}

pub fn enter_fullscreen() -> Result<FullscreenGuard> {
    execute!(io::stdout(), EnterAlternateScreen, EnableMouseCapture, Hide)?;
    Ok(FullscreenGuard)
}

impl Drop for FullscreenGuard {
    fn drop(&mut self) {
        let _ = terminal::disable_raw_mode();
        let _ = execute!(io::stdout(), DisableMouseCapture, Show, LeaveAlternateScreen);
    }
}

impl App {
    fn new(targets: Vec<ScanTarget>, min_bytes: u64, tx: Sender<scanner::ScanResult>) -> Self {
        let root_count = targets.len();
        let expanded = targets
            .iter()
            .map(|target| root_key(&target.root_display))
            .collect();

        Self {
            targets,
            expanded,
            selected: 0,
            should_quit: false,
            min_bytes,
            pane_ratio: 38,
            drawer_scroll: 0,
            show_help: false,
            status_message: format!(
                "Ready. {} roots loaded. Press ? for help.",
                root_count
            ),
            resizing: false,
            layout: LayoutCache::default(),
            tx,
        }
    }

    fn total_size(&self) -> u64 {
        self.targets.iter().map(|target| target.total_size).sum()
    }

    fn visible_items(&self) -> Vec<VisibleItem> {
        let mut items = Vec::new();

        for (target_idx, target) in self.targets.iter().enumerate() {
            let key = root_key(&target.root_display);
            let expanded = self.expanded.contains(&key);
            items.push(VisibleItem {
                key,
                title: target.root_display.clone(),
                depth: 0,
                expandable: !target.nodes.is_empty(),
                expanded,
                is_loading: false,
                kind: ItemKind::Target(target_idx),
            });

            if expanded {
                self.push_nodes(target_idx, &target.nodes, Vec::new(), 1, &mut items);
            }
        }

        items
    }

    fn push_nodes(
        &self,
        target_idx: usize,
        nodes: &[DirNode],
        parent_path: Vec<usize>,
        depth: usize,
        items: &mut Vec<VisibleItem>,
    ) {
        for (index, node) in nodes.iter().enumerate() {
            if node.size < self.min_bytes {
                continue;
            }

            let mut node_path = parent_path.clone();
            node_path.push(index);

            let key = node_key(node);
            let expanded = self.expanded.contains(&key);
            items.push(VisibleItem {
                key,
                title: node.name.clone(),
                depth,
                expandable: node.has_children,
                expanded,
                is_loading: node.is_loading,
                kind: ItemKind::Node {
                    target_idx,
                    node_path: node_path.clone(),
                },
            });

            if expanded {
                self.push_nodes(target_idx, &node.children, node_path, depth + 1, items);
            }
        }
    }

    fn clamp_selection(&mut self) {
        let len = self.visible_items().len();
        if len == 0 {
            self.selected = 0;
        } else if self.selected >= len {
            self.selected = len - 1;
        }
        self.ensure_selected_visible();
    }

    fn move_selection(&mut self, delta: isize) {
        let len = self.visible_items().len();
        if len == 0 {
            self.selected = 0;
            return;
        }

        let next = self.selected as isize + delta;
        self.selected = next.clamp(0, len.saturating_sub(1) as isize) as usize;
    }

    fn toggle_current(&mut self) {
        let items = self.visible_items();
        if let Some(item) = items.get(self.selected).cloned() {
            if !item.expandable {
                return;
            }

            if self.expanded.contains(&item.key) {
                self.expanded.remove(&item.key);
                self.set_status(format!("Collapsed {}", item.title));
            } else {
                self.ensure_item_loaded(&item.kind);
                self.expanded.insert(item.key.clone());
                self.set_status(format!("Expanded {}", item.title));
            }
        }
    }

    fn expand_current(&mut self) {
        let items = self.visible_items();
        if let Some(item) = items.get(self.selected).cloned() {
            if item.expandable {
                self.ensure_item_loaded(&item.kind);
                let inserted = self.expanded.insert(item.key.clone());
                if inserted {
                    self.set_status(format!("Expanded {}", item.title));
                }
            }
        }
    }

    fn collapse_current(&mut self) {
        let items = self.visible_items();
        let Some(item) = items.get(self.selected) else {
            return;
        };

        if item.expandable && self.expanded.contains(&item.key) {
            self.expanded.remove(&item.key);
            self.set_status(format!("Collapsed {}", item.title));
            return;
        }

        if let ItemKind::Node { target_idx, node_path } = &item.kind {
            if node_path.len() == 1 {
                let parent_key = root_key(&self.targets[*target_idx].root_display);
                if let Some(index) = items.iter().position(|candidate| candidate.key == parent_key) {
                    self.selected = index;
                    self.set_status(format!("Moved to parent {}", self.targets[*target_idx].root_display));
                }
            } else {
                let parent = self.node_at(*target_idx, &node_path[..node_path.len() - 1]);
                let parent_key = node_key(parent);
                let parent_name = parent.name.clone();
                if let Some(index) = items.iter().position(|candidate| candidate.key == parent_key) {
                    self.selected = index;
                    self.set_status(format!("Moved to parent {}", parent_name));
                }
            }
        }
    }

    fn handle_key(&mut self, code: KeyCode) {
        match code {
            KeyCode::Char('q') | KeyCode::Esc => self.should_quit = true,
            KeyCode::Down | KeyCode::Char('j') => self.move_selection(1),
            KeyCode::Up | KeyCode::Char('k') => self.move_selection(-1),
            KeyCode::Home => self.selected = 0,
            KeyCode::End => {
                let len = self.visible_items().len();
                if len > 0 {
                    self.selected = len - 1;
                }
            }
            KeyCode::Right | KeyCode::Char('l') | KeyCode::Char('e') => self.expand_current(),
            KeyCode::Left | KeyCode::Char('h') | KeyCode::Char('c') => self.collapse_current(),
            KeyCode::Enter | KeyCode::Char(' ') | KeyCode::Char('d') => self.toggle_current(),
            KeyCode::Char('?') => {
                self.show_help = !self.show_help;
                self.set_status(if self.show_help {
                    "Opened help drawer".to_string()
                } else {
                    "Closed help drawer".to_string()
                });
            }
            KeyCode::Char('+') | KeyCode::Char('=') | KeyCode::Char(']') => self.resize_panes(3),
            KeyCode::Char('-') | KeyCode::Char('_') | KeyCode::Char('[') => self.resize_panes(-3),
            _ => {}
        }
        self.ensure_selected_loaded();
        self.ensure_selected_visible();
    }

    fn handle_mouse(&mut self, mouse: MouseEvent) {
        match mouse.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                if self.is_on_divider(mouse.column) {
                    self.resizing = true;
                    self.set_ratio_from_column(mouse.column);
                    self.set_status(format!("Resizing panes: {}% / {}%", self.pane_ratio, 100 - self.pane_ratio));
                } else if point_in_rect(mouse.column, mouse.row, self.layout.drawers_area) {
                    if let Some(index) = self.drawer_index_at_row(mouse.row) {
                        if self.selected == index {
                            self.toggle_current();
                        } else {
                            self.selected = index;
                            let items = self.visible_items();
                            if let Some(item) = items.get(index) {
                                self.set_status(format!("Selected {}", item.title));
                            }
                        }
                    }
                } else if point_in_rect(mouse.column, mouse.row, self.layout.help_area) {
                    self.show_help = !self.show_help;
                    self.set_status(if self.show_help {
                        "Opened help drawer".to_string()
                    } else {
                        "Closed help drawer".to_string()
                    });
                }
            }
            MouseEventKind::Drag(MouseButton::Left) => {
                if self.resizing {
                    self.set_ratio_from_column(mouse.column);
                    self.set_status(format!("Resizing panes: {}% / {}%", self.pane_ratio, 100 - self.pane_ratio));
                }
            }
            MouseEventKind::Up(MouseButton::Left) => {
                if self.resizing {
                    self.resizing = false;
                    self.set_status(format!("Pane ratio set to {}% / {}%", self.pane_ratio, 100 - self.pane_ratio));
                }
            }
            MouseEventKind::ScrollDown => {
                if point_in_rect(mouse.column, mouse.row, self.layout.drawers_area) {
                    self.move_selection(1);
                }
            }
            MouseEventKind::ScrollUp => {
                if point_in_rect(mouse.column, mouse.row, self.layout.drawers_area) {
                    self.move_selection(-1);
                }
            }
            _ => {}
        }
        self.ensure_selected_loaded();
        self.ensure_selected_visible();
    }

    fn resize_panes(&mut self, delta: i16) {
        let next = (self.pane_ratio as i16 + delta).clamp(25, 65) as u16;
        self.pane_ratio = next;
        self.set_status(format!("Pane ratio set to {}% / {}%", self.pane_ratio, 100 - self.pane_ratio));
    }

    fn set_ratio_from_column(&mut self, column: u16) {
        let width = self.layout.drawers_area.width + self.layout.details_area.width;
        if width == 0 {
            return;
        }
        let relative = column.saturating_sub(self.layout.drawers_area.x);
        let ratio = ((relative as u32) * 100 / width as u32) as i16;
        self.pane_ratio = ratio.clamp(25, 65) as u16;
    }

    fn ensure_selected_visible(&mut self) {
        let visible_rows = self.layout.drawer_rows.max(1);
        if self.selected < self.drawer_scroll {
            self.drawer_scroll = self.selected;
        } else if self.selected >= self.drawer_scroll + visible_rows {
            self.drawer_scroll = self.selected + 1 - visible_rows;
        }
    }

    fn ensure_selected_loaded(&mut self) {
        let item = self.visible_items().get(self.selected).cloned();
        if let Some(item) = item {
            self.ensure_item_loaded(&item.kind);
        }
    }

    fn ensure_item_loaded(&mut self, kind: &ItemKind) {
        if let ItemKind::Node {
            target_idx,
            node_path,
        } = kind
        {
            let (needs_load, node_name, node_path_buf) = {
                let node = self.node_at_mut(*target_idx, node_path);
                let needs_load = !node.children_loaded && !node.is_loading;
                if needs_load {
                    node.is_loading = true;
                }
                (needs_load, node.name.clone(), node.path.clone())
            };

            if needs_load {
                self.set_status(format!("Loading {}...", node_name));
                scanner::spawn_load_children(
                    *target_idx,
                    node_path.clone(),
                    node_path_buf,
                    self.tx.clone(),
                );
            }
        }
    }

    fn handle_scan_result(&mut self, result: scanner::ScanResult) {
        let (key, has_children, node_name) = {
            let node = self.node_at_mut(result.target_idx, &result.node_path);
            node.children = result.children;
            node.has_children = !node.children.is_empty();
            node.children_loaded = true;
            node.is_loading = false;
            
            (node_key(node), node.has_children, node.name.clone())
        };
        
        // Only expand the loaded folder if it was already marked as expanded
        // OR it was in the process of expanding
        if self.expanded.contains(&key) && !has_children {
            self.expanded.remove(&key);
        }
        
        // Don't modify selection to stay friendly
        self.set_status(format!("Loaded {}", node_name));
    }

    fn drawer_index_at_row(&self, row: u16) -> Option<usize> {
        if self.layout.drawer_rows == 0 || row <= self.layout.drawers_area.y {
            return None;
        }

        let content_start = self.layout.drawers_area.y + 1;
        let relative = row.saturating_sub(content_start) as usize;
        if relative >= self.layout.drawer_rows {
            return None;
        }

        let index = self.drawer_scroll + relative;
        (index < self.visible_items().len()).then_some(index)
    }

    fn set_status(&mut self, status_message: String) {
        self.status_message = status_message;
    }

    fn is_on_divider(&self, column: u16) -> bool {
        is_near(column, self.layout.divider_x, 1)
    }

    fn detail_data(&self, item: &VisibleItem) -> DetailData {
        let global_total = self.total_size();

        match &item.kind {
            ItemKind::Target(target_idx) => {
                let target = &self.targets[*target_idx];
                let breakdown = child_breakdown(&target.nodes, target.total_size, self.min_bytes);
                let largest_child = target
                    .nodes
                    .iter()
                    .max_by_key(|child| child.size)
                    .map(|child| (child.name.clone(), child.size));

                DetailData {
                    title: target.root_display.clone(),
                    kind_label: "scan root",
                    path: target.root_display.clone(),
                    size: target.total_size,
                    root_label: target.root_display.clone(),
                    root_share: percentage(target.total_size, target.total_size),
                    total_share: percentage(target.total_size, global_total),
                    direct_children: target.nodes.len(),
                    visible_children: target
                        .nodes
                        .iter()
                        .filter(|child| child.size >= self.min_bytes)
                        .count(),
                    descendants: target.nodes.iter().map(count_descendants_inclusive).sum(),
                    tracked_depth: target.nodes.iter().map(tree_depth).max().unwrap_or(0),
                    largest_child,
                    breakdown,
                }
            }
            ItemKind::Node {
                target_idx,
                node_path,
            } => {
                let target = &self.targets[*target_idx];
                let node = self.node_at(*target_idx, node_path);
                let breakdown = child_breakdown(&node.children, node.size, self.min_bytes);
                let largest_child = node
                    .children
                    .iter()
                    .max_by_key(|child| child.size)
                    .map(|child| (child.name.clone(), child.size));

                DetailData {
                    title: node.name.clone(),
                    kind_label: "folder drawer",
                    path: node.path.display().to_string(),
                    size: node.size,
                    root_label: target.root_display.clone(),
                    root_share: percentage(node.size, target.total_size),
                    total_share: percentage(node.size, global_total),
                    direct_children: node.children.len(),
                    visible_children: node
                        .children
                        .iter()
                        .filter(|child| child.size >= self.min_bytes)
                        .count(),
                    descendants: count_descendants(node),
                    tracked_depth: tree_depth(node),
                    largest_child,
                    breakdown,
                }
            }
        }
    }

    fn node_at<'a>(&'a self, target_idx: usize, node_path: &[usize]) -> &'a DirNode {
        let mut node = &self.targets[target_idx].nodes[node_path[0]];
        for index in &node_path[1..] {
            node = &node.children[*index];
        }
        node
    }

    fn node_at_mut<'a>(&'a mut self, target_idx: usize, node_path: &[usize]) -> &'a mut DirNode {
        node_at_path_mut(&mut self.targets[target_idx].nodes, node_path)
    }

    fn render(&mut self, frame: &mut Frame, c: &Chars) {
        self.clamp_selection();
        self.ensure_selected_loaded();
        let items = self.visible_items();
        let selected_item = items.get(self.selected);
        let details = selected_item.map(|item| self.detail_data(item));
        let area = frame.area();
        let help_height = if self.show_help { 7 } else { 4 };

        let [header, body, footer] = Layout::vertical([
            Constraint::Length(3),
            Constraint::Min(10),
            Constraint::Length(help_height),
        ])
        .areas(area);

        let [left, right] = Layout::horizontal([
            Constraint::Percentage(self.pane_ratio),
            Constraint::Percentage(100 - self.pane_ratio),
        ])
        .areas(body);

        self.layout = LayoutCache {
            drawers_area: left,
            details_area: right,
            help_area: footer,
            divider_x: left.x + left.width,
            drawer_rows: left.height.saturating_sub(2) as usize,
        };
        self.ensure_selected_visible();

        self.render_header(frame, header, items.len(), c);
        self.render_drawers(frame, left, &items, c);
        self.render_details(frame, right, details.as_ref());
        self.render_footer(frame, footer, selected_item.map(|item| item.title.as_str()));
    }

    fn render_header(&self, frame: &mut Frame, area: Rect, visible_count: usize, c: &Chars) {
        let header = Paragraph::new(vec![Line::from(vec![
            Span::styled(
                format!("{} rusize explorer", c.folder),
                Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
            ),
            Span::raw("   "),
            Span::styled(
                format!("{} roots", self.targets.len()),
                Style::default().fg(Color::Yellow),
            ),
            Span::raw("   "),
            Span::styled(
                format!("{} visible items", visible_count),
                Style::default().fg(Color::Green),
            ),
        ])])
        .block(Block::default().borders(Borders::ALL).title("Overview"));

        frame.render_widget(header, area);
    }

    fn render_drawers(&self, frame: &mut Frame, area: Rect, items: &[VisibleItem], c: &Chars) {
        if items.is_empty() {
            frame.render_widget(
                Paragraph::new("No scan results available.")
                    .block(Block::default().borders(Borders::ALL).title("Drawers")),
                area,
            );
            return;
        }

        let visible_rows = area.height.saturating_sub(2) as usize;
        let list_items: Vec<Line> = items
            .iter()
            .enumerate()
            .skip(self.drawer_scroll)
            .take(visible_rows)
            .map(|(index, item)| {
                let indent = "  ".repeat(item.depth);
                
                let marker = if item.is_loading {
                    "[~]".to_string()
                } else {
                    drawer_marker(item.expandable, item.expanded, c).to_string()
                };

                let style = if item.depth == 0 {
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::White)
                };

                let row_style = if index == self.selected {
                    Style::default()
                        .bg(Color::Rgb(26, 44, 62))
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                };

                Line::from(vec![
                    Span::raw(indent),
                    Span::styled(marker, Style::default().fg(Color::Yellow)),
                    Span::raw(" "),
                    Span::styled(item.title.clone(), style),
                ])
                .style(row_style)
            })
            .collect();

        let title = format!(
            "Drawers  {}%  scroll {}  {}",
            self.pane_ratio,
            self.drawer_scroll,
            if self.resizing { "dragging" } else { "ready" }
        );
        let drawers = Paragraph::new(list_items)
            .block(Block::default().borders(Borders::ALL).title(title))
            .wrap(Wrap { trim: false });
        frame.render_widget(drawers, area);
    }

    fn render_details(&self, frame: &mut Frame, area: Rect, details: Option<&DetailData>) {
        let [summary_area, metrics_area, breakdown_area] = Layout::vertical([
            Constraint::Length(8),
            Constraint::Length(7),
            Constraint::Min(8),
        ])
        .areas(area);

        let Some(details) = details else {
            frame.render_widget(
                Paragraph::new("Select a drawer to inspect details.")
                    .block(Block::default().borders(Borders::ALL).title("Details")),
                area,
            );
            return;
        };

        let summary = Paragraph::new(vec![
            Line::from(vec![
                Span::styled(
                    details.title.clone(),
                    Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
                ),
                Span::raw("  "),
                Span::styled(details.kind_label, Style::default().fg(Color::Yellow)),
            ]),
            Line::from(format!("Path: {}", details.path)),
            Line::from(format!("Root: {}", details.root_label)),
            Line::from(format!("Size: {}", format_size(bytes_to_mb(details.size)))),
            Line::from(format!(
                "Children: {} direct / {} visible / {} descendants",
                details.direct_children, details.visible_children, details.descendants
            )),
            Line::from(format!("Tracked depth: {} levels", details.tracked_depth)),
        ])
        .block(Block::default().borders(Borders::ALL).title("Details"))
        .wrap(Wrap { trim: true });
        frame.render_widget(summary, summary_area);

        let [root_area, total_area] = Layout::horizontal([
            Constraint::Percentage(50),
            Constraint::Percentage(50),
        ])
        .areas(metrics_area);

        let root_gauge = Gauge::default()
            .block(Block::default().borders(Borders::ALL).title("Share of Root"))
            .gauge_style(Style::default().fg(Color::Green).bg(Color::Black))
            .percent(details.root_share.round().clamp(0.0, 100.0) as u16)
            .label(format!("{:.1}%", details.root_share));
        frame.render_widget(root_gauge, root_area);

        let total_gauge = Gauge::default()
            .block(Block::default().borders(Borders::ALL).title("Share of All Scanned Data"))
            .gauge_style(Style::default().fg(Color::Magenta).bg(Color::Black))
            .percent(details.total_share.round().clamp(0.0, 100.0) as u16)
            .label(format!("{:.1}%", details.total_share));
        frame.render_widget(total_gauge, total_area);

        let rows: Vec<Row> = if details.breakdown.is_empty() {
            vec![Row::new(vec![
                Cell::from("No child folders above the current threshold"),
                Cell::from("-"),
                Cell::from("-"),
            ])]
        } else {
            details
                .breakdown
                .iter()
                .map(|(name, pct, size)| {
                    Row::new(vec![
                        Cell::from(name.clone()),
                        Cell::from(format!("{pct:.1}%")),
                        Cell::from(format_size(bytes_to_mb(*size))),
                    ])
                })
                .collect()
        };

        let largest = details
            .largest_child
            .as_ref()
            .map(|(name, size)| format!("Largest child: {} ({})", name, format_size(bytes_to_mb(*size))))
            .unwrap_or_else(|| "Largest child: n/a".to_string());

        let table = Table::new(
            rows,
            [
                Constraint::Percentage(50),
                Constraint::Percentage(20),
                Constraint::Percentage(30),
            ],
        )
        .header(
            Row::new(vec!["Folder", "%", "Size"])
                .style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
        )
        .block(Block::default().borders(Borders::ALL).title(largest))
        .column_spacing(1);
        frame.render_widget(table, breakdown_area);
    }

    fn render_footer(&self, frame: &mut Frame, area: Rect, selected_title: Option<&str>) {
        let selected_title = selected_title.unwrap_or("nothing selected");
        let mut lines = vec![
            Line::from(vec![
                Span::styled("Status: ", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
                Span::raw(self.status_message.clone()),
            ]),
            Line::from(format!(
                "Selected: {}   panes: {}% / {}%   min {}   lazy recursion on   sort desc",
                selected_title,
                self.pane_ratio,
                100 - self.pane_ratio,
                format_size(bytes_to_mb(self.min_bytes)),
            )),
        ];

        if self.show_help {
            lines.push(Line::from(
                "Keys: q quit, arrows/jk move, c collapse, d toggle, e expand, ? help, +/- resize",
            ));
            lines.push(Line::from(
                "Mouse: click drawer to select, click selected drawer to toggle, wheel scrolls, drag divider resizes panes",
            ));
            lines.push(Line::from(
                "Tip: click the help drawer itself to collapse it and keep more space for the explorer.",
            ));
        } else {
            lines.push(Line::from("Press ? to expand the help drawer. Mouse drag on the center divider resizes panes."));
        }

        let footer = Paragraph::new(lines)
        .block(Block::default().borders(Borders::ALL).title(if self.show_help {
            "Status / Help"
        } else {
            "Status"
        }))
        .wrap(Wrap { trim: true });

        frame.render_widget(footer, area);
    }
}

// ---------------------------------------------------------------------------
// Banner
// ---------------------------------------------------------------------------

/// Print the application banner.
pub fn banner(c: &Chars) {
    let hz_line: String = c.hz.repeat(50);

    println!();
    println!(
        "{}",
        format!("{}{}{}", c.tl, hz_line, c.tr).bright_cyan().bold()
    );
    println!(
        "{}",
        format!(
            "{}  {} rusize -- Disk Scanner                       {}",
            c.vt, c.bolt, c.vt
        )
        .bright_cyan()
        .bold()
    );
    println!(
        "{}",
        format!(
            "{}  High-Speed  |  Multi-Threaded  |  Drawer View   {}",
            c.vt, c.vt
        )
        .bright_cyan()
        .bold()
    );
    println!(
        "{}",
        format!("{}{}{}", c.bl, hz_line, c.br).bright_cyan().bold()
    );
}

// ---------------------------------------------------------------------------
// System info
// ---------------------------------------------------------------------------

/// Print OS and privilege status.
pub fn system_info(c: &Chars) {
    let os = std::env::consts::OS;
    let is_admin = is_root::is_root();

    let status = if is_admin {
        "Yes".green().bold()
    } else {
        "No".red().bold()
    };

    println!(
        "\n{}  System: {}  |  Elevated: {}",
        c.system,
        os.bright_magenta().bold(),
        status
    );

    if !is_admin {
        println!(
            "{}",
            format!(
                "{}  Running without Sudo/Admin. Some folders may be skipped.",
                c.warn
            )
            .yellow()
            .dimmed()
        );
    }
}

// ---------------------------------------------------------------------------
// Tree printer
// ---------------------------------------------------------------------------

/// Print a single scan root header.
pub fn scan_header(root_display: &str, c: &Chars) {
    println!(
        "{}  {}",
        c.arrow.bright_white().bold(),
        format!("Scanning: {}", root_display).bright_cyan()
    );
}

pub fn run_app(
    targets: Vec<ScanTarget>,
    min_bytes: u64,
    c: &Chars,
    tx: Sender<scanner::ScanResult>,
    rx: Receiver<scanner::ScanResult>,
) -> Result<()> {
    terminal::enable_raw_mode()?;

    let backend = ratatui::backend::CrosstermBackend::new(io::stdout());
    let mut terminal = ratatui::Terminal::new(backend)?;
    terminal.clear()?;

    let mut app = App::new(targets, min_bytes, tx);

    loop {
        terminal.draw(|frame| app.render(frame, c))?;

        if event::poll(Duration::from_millis(50))? {
            match event::read()? {
                Event::Key(key) => {
                    if key.kind == KeyEventKind::Press {
                        app.handle_key(key.code);
                    }
                }
                Event::Mouse(mouse) => app.handle_mouse(mouse),
                Event::Resize(_, _) => {}
                _ => {}
            }
        }

        // Process any messages from background scanners
        while let Ok(result) = rx.try_recv() {
            app.handle_scan_result(result);
        }

        if app.should_quit {
            break;
        }
    }

    terminal::disable_raw_mode()?;
    terminal.clear()?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn root_key(root_display: &str) -> String {
    format!("root::{root_display}")
}

fn node_key(node: &DirNode) -> String {
    format!("node::{}", node.path.display())
}

fn point_in_rect(column: u16, row: u16, rect: Rect) -> bool {
    column >= rect.x
        && column < rect.x + rect.width
        && row >= rect.y
        && row < rect.y + rect.height
}

fn is_near(value: u16, target: u16, tolerance: u16) -> bool {
    value >= target.saturating_sub(tolerance) && value <= target.saturating_add(tolerance)
}

fn drawer_marker(expandable: bool, expanded: bool, c: &Chars) -> &'static str {
    if !expandable {
        return c.branch;
    }

    if c.branch == "|-- " {
        if expanded {
            "[-]"
        } else {
            "[+]"
        }
    } else if expanded {
        "▼"
    } else {
        "▶"
    }
}

fn node_at_path_mut<'a>(nodes: &'a mut [DirNode], node_path: &[usize]) -> &'a mut DirNode {
    let (head, tail) = node_path
        .split_first()
        .expect("node_path must contain at least one index");
    let node = &mut nodes[*head];

    if tail.is_empty() {
        node
    } else {
        node_at_path_mut(&mut node.children, tail)
    }
}

fn percentage(part: u64, total: u64) -> f64 {
    if total == 0 {
        0.0
    } else {
        part as f64 / total as f64 * 100.0
    }
}

fn bytes_to_mb(bytes: u64) -> f64 {
    bytes as f64 / 1024.0 / 1024.0
}

fn child_breakdown(nodes: &[DirNode], total_size: u64, min_bytes: u64) -> Vec<(String, f64, u64)> {
    let mut breakdown: Vec<(String, f64, u64)> = nodes
        .iter()
        .filter(|node| node.size >= min_bytes)
        .map(|node| (node.name.clone(), percentage(node.size, total_size), node.size))
        .collect();

    breakdown.sort_by(|left, right| right.2.cmp(&left.2));
    breakdown.truncate(8);
    breakdown
}

fn count_descendants(node: &DirNode) -> usize {
    node.children.len() + node.children.iter().map(count_descendants).sum::<usize>()
}

fn count_descendants_inclusive(node: &DirNode) -> usize {
    1 + count_descendants(node)
}

fn tree_depth(node: &DirNode) -> usize {
    if node.children.is_empty() {
        1
    } else {
        1 + node.children.iter().map(tree_depth).max().unwrap_or(0)
    }
}

/// Format a size in megabytes into a human-readable string (MB / GB / TB).
pub fn format_size(size_mb: f64) -> String {
    if size_mb >= 1_048_576.0 {
        format!("{:.2} TB", size_mb / 1_048_576.0)
    } else if size_mb >= 1024.0 {
        format!("{:.2} GB", size_mb / 1024.0)
    } else {
        format!("{:.2} MB", size_mb)
    }
}

