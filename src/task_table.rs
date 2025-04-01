//! Widget that forms the 'main view' of tasks and their status. Doesn't hold the tasks(!) because
//! then we'd have to move a bunch of business logic from the app - unlike TaskPicker which holds
//! all its state
use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Rect},
    style::{Color, Style, Stylize},
    widgets::{Cell, Row, StatefulWidget, Table, TableState},
};

use crate::tasks::{Task, TaskStatus};

#[derive(Debug)]
pub struct TaskTable {
    pub state: TableState,
}

impl Default for TaskTable {
    fn default() -> Self {
        Self {
            state: TableState::default().with_selected(0),
        }
    }
}

impl TaskTable {
    /// Selects the next item in the table, wrapping around.
    pub fn next(&mut self, num_rows: usize) {
        if num_rows == 0 {
            self.state.select(None);
            return;
        }
        let new_sel = match self.state.selected() {
            Some(old_sel) => {
                if old_sel >= num_rows - 1 {
                    0
                } else {
                    old_sel + 1
                }
            }
            None => 0, // Select the first item if nothing is selected
        };
        self.state.select(Some(new_sel));
    }

    /// Selects the previous item in the table, wrapping around.
    pub fn previous(&mut self, num_rows: usize) {
        if num_rows == 0 {
            self.state.select(None);
            return;
        }
        let new_sel = match self.state.selected() {
            Some(old_sel) => {
                if old_sel == 0 {
                    num_rows - 1
                } else {
                    old_sel - 1
                }
            }
            None => 0, // Select the first item if nothing is selected
        };
        self.state.select(Some(new_sel));
    }
}

/// Renders the TaskTable widget.
///
/// Needs the list of tasks to render the rows.
impl<'a> StatefulWidget for &'a mut TaskTable {
    type State = &'a Vec<Task>;

    fn render(self, area: Rect, buf: &mut Buffer, tasks: &mut Self::State) {
        let header = Row::new(vec![
            "ID",
            "Name",
            "Status",
            "Halt?",
            "Progress",
            "Start Time",
            "End Time",
            "Description",
        ])
        .style(Style::new().bold()) // Example style
        .height(1);

        let mut row_ctr = 0;
        let rows: Vec<Row> = tasks // Use the state variable name `tasks`
            .iter()
            .map(|task| {
                row_ctr += 1;
                row_style(
                    Row::new(vec![
                        Cell::from(task.id.to_string()),
                        Cell::from(task.name),
                        status_cell_style(&task.status),
                        abort_cell_style(&task.status, task.pending_cancel),
                        Cell::from(format!("{}%", task.progress)),
                        Cell::from(task.start.format("%I:%M:%S %P").to_string()),
                        Cell::from(match task.end {
                            Some(time) => time.format("%I:%M:%S %P").to_string(),
                            None => "-".to_string(),
                        }),
                        Cell::from(task.description),
                    ]),
                    row_ctr,
                )
            })
            .collect();

        let widths = [
            //TODO: These could be made dynamic
            Constraint::Max(4),
            Constraint::Length(16),
            Constraint::Length(10),
            Constraint::Length(7),
            Constraint::Length(12),
            Constraint::Length(14),
            Constraint::Length(14),
            Constraint::Min(42), // Use Min for the last one to fill space
        ];

        // The block is now rendered by the App, we only render the table itself
        let table = Table::new(rows, widths)
            .header(header)
            .style(Color::White)
            .row_highlight_style(Style::new().on_blue().bold())
            //.column_highlight_style(Color::White)
            .cell_highlight_style(Style::new().reversed())
            .highlight_symbol(">> "); // Example highlight symbol

        // Use StatefulWidget's render method
        StatefulWidget::render(table, area, buf, &mut self.state);
    }
}

fn status_cell_style(status: &TaskStatus) -> Cell {
    let cell = Cell::from(status.to_string());
    match status {
        TaskStatus::Sleeping => cell.style(Color::Gray),
        TaskStatus::Finished => cell.style(Color::Green),
        TaskStatus::OnStrike => cell.style(Color::Red).slow_blink(),
        TaskStatus::Running => cell.style(Color::White),
        _ => cell,
    }
}

fn abort_cell_style(status: &TaskStatus, cancel: bool) -> Cell {
    if cancel {
        match status {
            TaskStatus::Canceled => Cell::from("Done").style(Color::Green),
            _ => Cell::from("Req").style(Color::Yellow),
        }
    } else {
        Cell::from(" ")
    }
}

// Could do more, but enforces alternating color
fn row_style(row: Row, ctr: i32) -> Row {
    if ctr % 2 == 0 {
        row
    } else {
        row.style(Color::Gray)
    }
}
