use std::iter::Inspect;

use color_eyre::eyre::{Ok, Result};
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind};
use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Rect},
    style::{Style, Stylize},
    symbols::border,
    text::Line,
    widgets::{
        Block, Borders, Cell, Clear, Paragraph, Row, StatefulWidget, Table, TableState, Widget,
    },
    DefaultTerminal, Frame, Viewport,
};
use task_picker::TaskPicker;
use tracing::{info, trace, Level};
mod task_picker;
mod tasks;

#[tokio::main]
async fn main() -> Result<()> {
    color_eyre::install()?;

    let appender = tracing_appender::rolling::never("./", "log");
    let (non_blocking, _guard) = tracing_appender::non_blocking(appender);
    tracing_subscriber::fmt()
        .with_writer(non_blocking)
        .with_thread_ids(true)
        .with_max_level(Level::TRACE)
        .init();
    info!("starting application");
    let mut terminal = ratatui::init();
    let result = App::default().run(&mut terminal);
    info!("application terminated. restoring. result: {:?}", &result);
    ratatui::restore();
    println!("Goodbye!");
    result
}

#[derive(Debug)]
pub struct App {
    picker: TaskPicker,
    table_state: TableState,
    tasks: Vec<tasks::Task>,
    view_state: ViewState,
    exit: bool,
}

#[derive(Debug)]
enum ViewState {
    TaskAdd,
    Monitor,
    Inspect,
}

impl Default for App {
    fn default() -> Self {
        Self {
            picker: TaskPicker::default(),
            table_state: TableState::default().with_selected(0),
            tasks: vec![],
            view_state: ViewState::Monitor,
            exit: false,
        }
    }
}

impl App {
    pub fn run(&mut self, terminal: &mut DefaultTerminal) -> Result<()> {
        while !self.exit {
            terminal.draw(|frame| self.view(frame))?;
            self.update()?;
        }
        Ok(())
    }

    fn view(&mut self, frame: &mut Frame) {
        let area = frame.area();
        frame.render_widget(self, area);
    }

    fn update(&mut self) -> Result<()> {
        match event::read()? {
            Event::Key(event) if event.kind == KeyEventKind::Press => self.handle_key_event(event),
            _ => {}
        };
        Ok(())
    }

    fn handle_key_event(&mut self, event: KeyEvent) {
        trace!("key down: {:?}", event);
        match event.code {
            KeyCode::Char('q') => {
                todo!()
            }
            KeyCode::Char('k') | KeyCode::Up => match self.view_state {
                ViewState::TaskAdd => self.picker.previous(),
                ViewState::Inspect => {} //TODO: This
                _ => {}
            },
            KeyCode::Char('j') | KeyCode::Down => match self.view_state {
                ViewState::TaskAdd => self.picker.next(),
                ViewState::Inspect => {} //TODO: This
                _ => {}
            },

            KeyCode::Enter => match self.view_state {
                ViewState::TaskAdd => {}
                ViewState::Inspect => {}
                ViewState::Monitor => {}
            },

            //Go to task add IFF we're at main menu
            KeyCode::F(1) => {
                match self.view_state {
                    ViewState::TaskAdd => {}
                    ViewState::Monitor | ViewState::Inspect => {
                        self.view_state = ViewState::TaskAdd;
                        self.picker.regen(); // Pick fresh pool entries
                    }
                };
            }
            KeyCode::F(2) => {
                todo!()
            }
            KeyCode::F(3) => self.exit(),

            // Go back unless we're @ main menu
            KeyCode::Esc => match self.view_state {
                ViewState::TaskAdd | ViewState::Inspect => self.view_state = ViewState::Monitor,
                ViewState::Monitor => {}
            },
            _ => {}
        }
    }

    fn exit(&mut self) {
        self.exit = true;
    }
}

impl Widget for &mut App {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let title = Line::from("  clustrctrl  ".bold());
        // TODO: Better represent what's actionable at a given time
        let controls = Line::from(match self.view_state {
            ViewState::Monitor => vec![
                " New Task ".into(),
                "<F1>".blue().bold(),
                " Monitor ".into(),
                "<F2>".blue().bold(),
                " Quit ".into(),
                "<F3> ".blue().bold(),
            ],
            ViewState::TaskAdd | ViewState::Inspect => vec![
                " Back ".into(),
                "<ESC>".blue().bold(),
                " New Task ".into(),
                "<F1>".blue().bold(),
                " Monitor ".into(),
                "<F2>".blue().bold(),
                " Quit ".into(),
                "<F3> ".blue().bold(),
            ],
        });
        /*
        let nav_controls = Line::from(vec![
            " Kill Task ".into(),
            "<Q>".blue().bold(),
            " Up ".into(),
            "<J>".blue().bold(),
            " Down ".into(),
            "<K>".blue().bold(),
        ]);
        */
        let block = Block::bordered()
            .title(title.left_aligned())
            .title_bottom(controls.centered())
            .border_set(border::THICK);

        // TABLE
        let rows: Vec<Row> = self
            .tasks
            .iter()
            .map(|task| {
                Row::new(vec![
                    //TODO: We can be more efficient here (CoW?)
                    Cell::from(task.id.to_string()),
                    Cell::from(task.name.clone()),
                    Cell::from(task.status.to_string()),
                    Cell::from("TODO".to_string()), // Time start TODO: Use time crate
                    Cell::from("TODO".to_string()), // Time end
                    Cell::from(task.description.to_string()),
                ])
            })
            .collect();

        let header = Row::new(vec!["ID", "Name", "Status", "Start Time", "End Time"]);
        let widths = [
            //TODO: These could be made dynamic
            Constraint::Length(8),
            Constraint::Length(10),
            Constraint::Length(10),
            Constraint::Length(16),
            Constraint::Length(16),
        ];
        let table = Table::new(rows, widths).header(header).block(block);
        StatefulWidget::render(table, area, buf, &mut self.table_state);

        // We want to draw our modal over if we're in add state
        if let ViewState::TaskAdd = self.view_state {
            let modal_width = (area.width as f32 * 0.85) as u16;
            let modal_height = (area.height as f32 * 0.85) as u16;
            //let modal_width = area.width / 2;
            //let modal_height = area.height / 2;
            let modal_area = Rect {
                x: (area.width - modal_width) / 2,
                y: (area.height - modal_height) / 2,
                width: modal_width,
                height: modal_height,
            };
            trace!("rendering modal with {:?}", modal_area);
            self.picker.render(modal_area, buf);
        }
    }
}
