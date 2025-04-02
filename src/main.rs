use std::time::Duration;

use color_eyre::eyre::Result;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind};
use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Layout, Rect},
    style::{Color, Style, Stylize},
    symbols::border,
    text::Line,
    widgets::{Block, Padding, StatefulWidget, Widget},
    DefaultTerminal, Frame,
};
use task_picker::{CandidateTask, TaskPicker};
use task_table::TaskTable;
use tasks::{Task, TaskRxMsg, TaskStatus, TaskTxMsg};
use tokio::{
    sync::{broadcast, mpsc},
    task,
};
use tracing::{error, info, trace, warn};
use tracing_subscriber::{
    fmt::format::FmtSpan, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter, Layer,
};
use tui_logger::{TuiLoggerLevelOutput, TuiLoggerWidget};
mod task_picker;
mod task_table;
mod tasks;

#[tokio::main]
async fn main() -> Result<()> {
    color_eyre::install()?;
    let appender = tracing_appender::rolling::never("./", "log");
    let (non_blocking, _guard) = tracing_appender::non_blocking(appender);
    tracing_subscriber::registry()
        .with(tui_logger::TuiTracingSubscriberLayer)
        .with(
            tracing_subscriber::fmt::layer()
                .with_writer(non_blocking)
                .with_thread_ids(true)
                .with_span_events(FmtSpan::NEW | FmtSpan::CLOSE)
                .with_filter(
                    EnvFilter::try_from_default_env().unwrap_or_else(|_| {
                        format!("{}=trace,info", env!("CARGO_CRATE_NAME")).into()
                    }),
                ),
        )
        .init();
    tui_logger::init_logger(tui_logger::LevelFilter::Info).unwrap();
    info!("starting application");
    match tokio::spawn(launch_app()).await? {
        Ok(_) => {}
        Err(e) => error!("error during app termination {e}"),
    };
    info!("application terminated. restoring");
    ratatui::restore();
    //TODO: Skill issue not using collaborative tasks. We could just force stop them probably
    println!("Goodbye! Any active tasks sent exit signals. This will take time to be heeded.");
    Ok(())
}

async fn launch_app() -> Result<()> {
    let mut terminal = ratatui::init();
    App::default().run(&mut terminal).await
}

#[derive(Debug)]
pub struct App {
    picker: TaskPicker,
    task_table: TaskTable,
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
    /// Modal should be active, and we can add tasks here
    TaskAdd,
    /// Main screen. Can't do anything but enter other modes & watch
    Monitor,
    /// Main screen, but we can select tasks on the table and cancel them
    Inspect,
}

impl Default for App {
    fn default() -> Self {
        // Used by tasks to bubble a message up
        let (mpsc_tx, mpsc_rx) = mpsc::channel(100);
        let (bcast_tx, _) = broadcast::channel(16);
        Self {
            picker: TaskPicker::default(),
            task_table: TaskTable::default(),
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
                //FIXME: We'd panic here if we got a message for an ID that doesn't exist
                // the logic is pretty tight where we TX but this would be !Ok in a srs project
                TaskTxMsg::RunReport { id, progress } => {
                    trace!("got a run report from {id} with progress {progress}%");
                    self.tasks[id].progress = progress;
                    self.tasks[id].status = TaskStatus::Running;
                }
                TaskTxMsg::SleepReport(id) => {
                    trace!("got a sleep report from {id}");
                    self.tasks[id].status = TaskStatus::Sleeping;
                }
                //TODO: Implement
                TaskTxMsg::LaborDispute(id) => {
                    info!("task {id} refuses to work at this time");
                    self.tasks[id].status = TaskStatus::OnStrike;
                }
                TaskTxMsg::Reconciliation(id) => {
                    info!("task {id} has reached an agreement, and will resume");
                    self.tasks[id].status = TaskStatus::Running;
                }
                TaskTxMsg::CancelReport(id) => {
                    info!("task {id} has sent word of termination");
                    self.tasks[id].status = TaskStatus::Canceled;
                }
            };
        }
        // Separately, check handles. This is kind of redundant given we have an MPSC channel that
        // reports doneness. Architectural skill issue, in hindsight.
        for task in self.tasks.iter_mut() {
            if let Some(handle) = task.check_done() {
                match handle.await {
                    Ok(res) => {
                        if let Some(sum) = res {
                            info!("task {} finished and reported: {sum}", task.id)
                        } else {
                            warn!(
                                "task {} finished after termination and reported no sum",
                                task.id
                            )
                        }
                    }
                    Err(e) => {
                        error!(
                            "problem finishing allegedly completed task {}: {e:?}",
                            task.id
                        );
                    }
                }
            }
        }
        Ok(())
    }

    fn handle_key_event(&mut self, event: KeyEvent) {
        trace!("key down: {:?}", event);
        match event.code {
            KeyCode::Char('k') | KeyCode::Up => match self.view_state {
                ViewState::TaskAdd => self.picker.previous(),
                ViewState::Inspect => self.task_table.previous(self.tasks.len()),
                ViewState::Monitor => {}
            },
            KeyCode::Char('j') | KeyCode::Down => match self.view_state {
                ViewState::TaskAdd => self.picker.next(),
                ViewState::Inspect => self.task_table.next(self.tasks.len()),
                ViewState::Monitor => {}
            },

            KeyCode::Char('r') => {
                if let ViewState::TaskAdd = self.view_state {
                    self.add_task(self.picker.select_random());
                }
            }

            KeyCode::Enter => match self.view_state {
                ViewState::TaskAdd => self.add_task(self.picker.select()),
                ViewState::Inspect => self.cancel_selected_task(),
                ViewState::Monitor => {}
            },

            //Go to task add IFF we're at main menu
            KeyCode::F(1) => {
                match self.view_state {
                    ViewState::TaskAdd | ViewState::Inspect => {}
                    ViewState::Monitor => {
                        self.view_state = ViewState::TaskAdd;
                        self.picker.regen(); // Pick fresh pool entries
                    }
                };
            }
            // Go to inspect mode IFF we're at main menu
            KeyCode::F(2) => match self.view_state {
                ViewState::TaskAdd | ViewState::Inspect => {}
                ViewState::Monitor => {
                    self.view_state = ViewState::Inspect;
                    // If table is not empty and nothing selected, select first row
                    if !self.tasks.is_empty() && self.task_table.state.selected().is_none() {
                        self.task_table.state.select(Some(0));
                    }
                }
            },

            // We can always exit
            KeyCode::F(3) => self.exit(),

            // Go back unless we're @ main menu
            KeyCode::Esc => match self.view_state {
                ViewState::TaskAdd | ViewState::Inspect => {
                    self.view_state = ViewState::Monitor;
                    self.task_table.state.select(None);
                }
                ViewState::Monitor => {}
            },
            _ => {}
        }
    }

    /// Calls out for the actual task, mostly handles UI juggling
    fn add_task(&mut self, ct: Option<&'static CandidateTask>) {
        if let Some(ct) = ct {
            info!("selected candidate task {:?}", ct);
            self.view_state = ViewState::Monitor;
            self.tasks.push(Task::new(
                ct,
                self.mpsc_tx.clone(),
                self.bcast_tx.subscribe(),
                self.tasks_created, //This counter becomes the unique 'ID'
            ));
            self.tasks_created += 1;
        } else {
            //Should be recoverable so we'll just ignore it otherwise
            error!("attempted to select task from picker but got none");
        }
    }

    fn cancel_selected_task(&mut self) {
        // This only works because we don't have sorting TODO: Make less brittle?
        if let Some(selected) = self.task_table.state.selected() {
            // Use get_mut to obtain a mutable reference directly
            if let Some(task) = self.tasks.get_mut(selected) {
                match self.bcast_tx.send(TaskRxMsg::PleaseStop(task.id)) {
                    Ok(_) => {
                        info!("sent a cancel message to task {}", task.id);
                        task.pending_cancel = true;
                    }
                    Err(e) => error!("problem sending cancel message to task {}: {e:?}", task.id),
                }
                return;
            }
        }
        warn!("tried to send a cancel message to a task that doesn't exist");
    }

    fn exit(&mut self) {
        //TODO: Worst case this broadcast has a 60 second delay, not great for exiting!
        match self.bcast_tx.send(TaskRxMsg::EveryoneStopPls) {
            Ok(_) => info!("sent cancel message to all tasks"),
            Err(e) => error!("problem sending cancel message to all tasks {e:?}"),
        }
        self.exit = true;
    }
}

impl Widget for &mut App {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let title = match self.view_state {
            ViewState::Monitor => Line::from("  clustrctrl  ".bold()),
            ViewState::Inspect => Line::from("  clustrctrl ━ [inspect] ".bold()),
            ViewState::TaskAdd => Line::from("  clustrctrl ━ [task add] ".bold()),
        };
        let controls = Line::from(match self.view_state {
            ViewState::Monitor => vec![
                " New Task ".into(),
                "<F1>".blue().bold(),
                " Manage Tasks ".into(),
                "<F2>".blue().bold(),
                " Quit ".into(),
                "<F3> ".blue().bold(),
            ],
            ViewState::TaskAdd => vec![
                " Back ".into(),
                "<ESC>".blue().bold(),
                " Quit ".into(),
                "<F3> ".blue().bold(),
            ],
            ViewState::Inspect => vec![
                " Back ".into(),
                "<ESC>".blue().bold(),
                " Terminate Task ".into(),
                "<ENTER>".blue().bold(),
                " Quit ".into(),
                "<F3> ".blue().bold(),
            ],
        });

        let main_block = Block::bordered()
            .title(title.left_aligned())
            .title_bottom(controls.centered())
            .border_set(border::THICK)
            .padding(Padding::new(2, 2, 1, 4));

        // Render the main block first to draw the borders
        let internal_area = main_block.inner(area);
        main_block.render(area, buf);

        // Table fits to tasks + padding, or takes the whole window if we're short on room
        let table_height = ((self.tasks.len() + 6) as u16).min(internal_area.height);
        let [table_area, logger_area] = Layout::vertical([
            Constraint::Length(table_height),
            Constraint::Min(0), // If there's leftovers, logger gets it
        ])
        .areas(internal_area);
        // Render the TaskTable inside the main block's inner area
        // Pass the task data required by the TaskTable widget's render method
        StatefulWidget::render(
            &mut self.task_table,
            table_area,
            buf,
            &mut &self.tasks, // We don't mutate but the trait wants a mut ref
        );

        // Render the TuiLogger in remaining space
        if logger_area.area() > 0 {
            // Mostly lifted from the example code
            TuiLoggerWidget::default()
                .block(
                    Block::bordered()
                        .title(" Message Stream ")
                        .padding(Padding::uniform(1)),
                )
                .style_debug(Style::default().fg(Color::Green))
                .style_warn(Style::default().fg(Color::Yellow))
                .style_trace(Style::default().fg(Color::Magenta))
                .style_info(Style::default().fg(Color::Cyan))
                .output_separator('|')
                .output_timestamp(Some("%H:%M:%S%.3f ".to_string()))
                .output_level(Some(TuiLoggerLevelOutput::Long))
                .output_target(false)
                .output_file(false)
                .output_line(false)
                .style(Style::default().fg(Color::White))
                .render(logger_area, buf);
        }

        // We want to draw our modal over if we're in add state
        // TODO: Put all this inside render() if it gets more complicated
        if let ViewState::TaskAdd = self.view_state {
            let modal_width = (area.width as f32 * 0.85) as u16;
            let modal_height = (task_picker::FETCH_AMOUNT + 2) as u16;
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
