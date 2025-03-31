use std::time::Duration;

use color_eyre::eyre::Result;
use crossterm::event::{self, Event, EventStream, KeyCode, KeyEvent, KeyEventKind};
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
use task_picker::{CandidateTask, TaskPicker};
use tasks::{Task, TaskRxMsg, TaskStatus, TaskTxMsg};
use tokio::{
    sync::{broadcast, mpsc},
    task,
};
use tracing::{error, info, instrument, trace, Level};
use tracing_subscriber::{fmt::format::FmtSpan, layer::SubscriberExt, util::SubscriberInitExt};
mod task_picker;
mod tasks;

///Used as a timeout to poll tasks if there's no events sooner
const TICK_FPS: f64 = 30.0;

#[tokio::main]
async fn main() -> Result<()> {
    color_eyre::install()?;
    let console = console_subscriber::spawn();
    let appender = tracing_appender::rolling::never("./", "log");
    let (non_blocking, _guard) = tracing_appender::non_blocking(appender);
    tracing_subscriber::registry()
        .with(console)
        .with(
            tracing_subscriber::fmt::layer()
                .with_writer(non_blocking)
                .with_thread_ids(true)
                .with_span_events(FmtSpan::NEW | FmtSpan::CLOSE),
        )
        .init();
    info!("starting application");
    match tokio::spawn(launch_app()).await? {
        Ok(_) => {}
        Err(e) => error!("error during app termination {e}"),
    };
    info!("application terminated. restoring");
    ratatui::restore();
    println!("Goodbye!");
    Ok(())
}

async fn launch_app() -> Result<()> {
    let mut terminal = ratatui::init();
    App::default().run(&mut terminal).await
}

#[derive(Debug)]
pub struct App {
    picker: TaskPicker,
    table_state: TableState,
    view_state: ViewState,
    exit: bool,
    tasks: Vec<tasks::Task>,
    tasks_created: tasks::Id, // Tokio ID's will be reused. We don't want that!
    // Tasks send us updates through this
    mpsc_rx: mpsc::Receiver<TaskTxMsg>,
    mpsc_tx: mpsc::Sender<TaskTxMsg>,
    // We send tasks orders through this
    bcast_tx: broadcast::Sender<TaskRxMsg>,
}

#[derive(Debug)]
enum ViewState {
    TaskAdd,
    Monitor,
    Inspect,
}

impl Default for App {
    fn default() -> Self {
        // Used by tasks to bubble a message up
        let (mpsc_tx, mut mpsc_rx) = mpsc::channel(100);
        let (bcast_tx, _) = broadcast::channel(16);
        Self {
            picker: TaskPicker::default(),
            table_state: TableState::default().with_selected(0),
            tasks: vec![],
            tasks_created: 0,
            view_state: ViewState::Monitor,
            exit: false,
            mpsc_rx,
            mpsc_tx,
            bcast_tx,
        }
    }
}

impl App {
    pub async fn run(&mut self, terminal: &mut DefaultTerminal) -> Result<()> {
        while !self.exit {
            terminal.draw(|frame| self.view(frame))?;
            self.update().await?;
            task::yield_now().await;
        }
        Ok(())
    }

    fn view(&mut self, frame: &mut Frame) {
        let area = frame.area();
        frame.render_widget(self, area);
    }

    #[instrument(skip(self))]
    async fn update(&mut self) -> Result<()> {
        //If I were doing it all over again I'd use a proper event-driven architecture
        //Like in the templates
        if event::poll(Duration::from_millis(500))? {
            match event::read()? {
                Event::Key(event) if event.kind == KeyEventKind::Press => {
                    self.handle_key_event(event)
                }
                _ => {}
            };
        }
        // Check our messages, and see if any task is done
        // Legally speaking, this is struct and tokio abuse.
        while let Ok(msg) = self.mpsc_rx.try_recv() {
            match msg {
                TaskTxMsg::RunReport { id, progress } => {
                    trace!("got a run report from {id} with progress {progress}%");
                    self.tasks[id].progress = progress;
                    self.tasks[id].status = TaskStatus::Running;
                }
                TaskTxMsg::SleepReport(id) => {
                    trace!("got a sleep report from {id}");
                    self.tasks[id].status = TaskStatus::Sleeping;
                }
                TaskTxMsg::LaborDispute(id) => {
                    trace!("task {id} refuses to work at this time");
                    self.tasks[id].status = TaskStatus::OnStrike;
                }
                TaskTxMsg::Reconciliation(id) => {
                    trace!("task {id} has reached an agreement, and will resume");
                    self.tasks[id].status = TaskStatus::Running;
                }
            };
        }
        // FIXME: Rethink the polling here
        self.tasks.iter_mut().for_each(|task| async {
            if let Some(i) = task.poll().await {
                info!("task {} finished and reported {i}", task.id);
            }
        });
        Ok(())
    }

    fn handle_key_event(&mut self, event: KeyEvent) {
        trace!("key down: {:?}", event);
        match event.code {
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
                ViewState::TaskAdd => {
                    if let Some(ct) = self.picker.select() {
                        info!("selected candidate task {:?}", ct);
                        self.add_task(ct);
                        self.view_state = ViewState::Monitor;
                    } else {
                        //Should be recoverable so we'll just ignore it otherwise
                        error!("attempted to select task from picker but got none");
                    }
                }
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

    /// Calls out for the actual task, mostly handles UI juggling
    fn add_task(&mut self, ct: &CandidateTask) {
        self.tasks.push(Task::new(
            ct,
            self.mpsc_tx.clone(),
            self.bcast_tx.subscribe(),
            self.tasks_created, //This counter becomes the unique 'ID'
        ));
        self.tasks_created += 1;
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
                " Kill Selected ".into(),
                "<F2>".blue().bold(),
                " Quit ".into(),
                "<F3> ".blue().bold(),
            ],
            ViewState::TaskAdd | ViewState::Inspect => vec![
                " Back ".into(),
                "<ESC>".blue().bold(),
                " New Task ".into(),
                "<F1>".blue().bold(),
                " Kill Selected ".into(), // Not actionable
                "<F2>".blue().bold(),
                " Quit ".into(),
                "<F3> ".blue().bold(),
            ],
        });

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
                    Cell::from(task.name),
                    Cell::from(task.status.to_string()),
                    Cell::from(format!("{}%", task.progress)),
                    Cell::from(task.start.format("%I:%M:%S %P").to_string()),
                    Cell::from(match task.end {
                        Some(time) => time.format("%I:%M:%S %P").to_string(),
                        None => "-".to_string(),
                    }),
                    Cell::from(task.description),
                ])
            })
            .collect();

        let header = Row::new(vec![
            "ID",
            "Name",
            "Status",
            "Progress",
            "Start Time",
            "End Time",
            "Description",
        ]);
        let widths = [
            //TODO: These could be made dynamic
            Constraint::Length(8),
            Constraint::Length(10),
            Constraint::Length(10),
            Constraint::Length(12),
            Constraint::Length(16),
            Constraint::Length(16),
            Constraint::Length(42),
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
