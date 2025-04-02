use crate::task_picker::CandidateTask;
use chrono::{DateTime, Local};
use std::thread::sleep;
use std::time::Duration;
use std::{fmt, mem};
use tokio::sync::broadcast::error::TryRecvError;
use tokio::sync::{broadcast, mpsc};
use tokio::task::{self, JoinHandle};
use tracing::{error, info, instrument, trace, warn};

pub type Id = usize;

#[derive(Debug)]
pub struct Task {
    pub id: Id,
    pub name: &'static str,
    pub status: TaskStatus,
    pub start: DateTime<Local>,
    pub end: Option<DateTime<Local>>,
    pub description: &'static str,
    pub handle: Option<JoinHandle<Option<i128>>>,
    pub progress: u8, // This is the part where I regretted not just sharing the struct w/ task
    pub pending_cancel: bool,
}

#[derive(Debug)]
pub enum TaskStatus {
    Running,
    Sleeping,
    OnStrike,
    KnownUnknown,
    Finished,
    Canceled,
}

/// Sent from tasks via mpsc to App
#[derive(Debug)]
pub enum TaskTxMsg {
    /// Conditions were untenable and the task refuses to work
    LaborDispute(Id),
    /// Work resumes after a bargain was struck
    Reconciliation(Id),
    /// Updates the table with percentage (progress is 0..100)
    RunReport {
        id: Id,
        progress: u8,
    },
    SleepReport(Id),
    CancelReport(Id),
}

/// Sent by App to all tasks via broadcast (tasks check if it's for them)
#[derive(Debug, Clone, Copy)]
pub enum TaskRxMsg {
    PleaseStop(Id), // Abort handles don't work on sync spawns
    EveryoneStopPls,
}

impl fmt::Display for TaskStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TaskStatus::Running => write!(f, "Running"),
            TaskStatus::Sleeping => write!(f, "Sleeping"),
            TaskStatus::OnStrike => write!(f, "Strike!"),
            TaskStatus::KnownUnknown => write!(f, "???"),
            TaskStatus::Finished => write!(f, "Done"),
            TaskStatus::Canceled => write!(f, "Cancelled"),
        }
    }
}

impl Task {
    pub fn new(
        ct: &CandidateTask,
        tx: mpsc::Sender<TaskTxMsg>,
        rx: broadcast::Receiver<TaskRxMsg>,
        id: Id,
    ) -> Self {
        // This is write once read never nonsense because I only wanted so much effort
        // into the 'pretend to work' code
        let start = Local::now();
        let mut proto_self = Self {
            id,
            name: ct.name,
            status: TaskStatus::KnownUnknown,
            start,
            end: None,
            description: ct.description,
            handle: None,
            progress: 0,
            pending_cancel: false,
        };
        let handle = task::spawn_blocking(move || Task::blocking_dummy_task(id, tx, rx));
        proto_self.handle = Some(handle);
        proto_self
    }
    pub fn check_done(&mut self) -> Option<JoinHandle<Option<i128>>> {
        if self.handle.as_ref().map_or(false, |h| h.is_finished()) {
            if !matches!(self.status, TaskStatus::Canceled) {
                // Cancel message will usually arrive first - don't let this over-write it!
                // This was fun to debug... Architectural skill issue
                self.status = TaskStatus::Finished;
            }
            self.end = Some(chrono::Local::now());
            self.progress = 100;
            // This is feels messy but the point is we want to lose ownership of the handle
            // We don't need any useful value stored in self.handle anymore since it's done
            let handle = mem::take(&mut self.handle).unwrap();
            self.handle = None;
            Some(handle)
        } else {
            // There is no handle or it isn't done
            None
        }
    }

    // The fact that these are static methods is symptomatic of undercooked architecture - I'm just
    // going to stick with message passing, but I'd lean towards shared state if I did it over

    /// This is the actual task we spawn
    /// Panics: Maybe
    /// Returns: Some(i128) if completed, or None if aborted by message
    #[instrument(skip(tx, rx))]
    fn blocking_dummy_task(
        id: Id,
        tx: mpsc::Sender<TaskTxMsg>,
        mut rx: broadcast::Receiver<TaskRxMsg>,
    ) -> Option<i128> {
        // The game was rigged all along
        let time_to_sleep = rand::random_range(2..60);
        let mut remaining_time = time_to_sleep;
        trace!("total sleep scheduled: {:?}", time_to_sleep);
        let mut sum: i128 = 0;
        while remaining_time > 0 {
            if Task::check_for_term_message(id, &mut rx, &tx) {
                return None;
            }
            // Do some really hecking important work
            trace!("sum: {:?}", sum);
            if let Err(some) = tx.blocking_send(TaskTxMsg::RunReport {
                id,
                //Sub-optimal casts but they keep us from rounding progress into 0%
                progress: (((time_to_sleep - remaining_time) as f64 / time_to_sleep as f64) * 100.0)
                    as u8,
            }) {
                error!("problem sending to App: {:?}", some);
            } else {
                trace!("sent a run report");
            }
            sum = rand::random_iter::<i32>()
                .take(111333777)
                .fold(sum, |acc, num| acc + ((num as i128 % 500).abs()));
            let microsleep = rand::random_range(1..(remaining_time + 1));
            remaining_time -= microsleep;
            if Task::check_for_term_message(id, &mut rx, &tx) {
                return None;
            }
            trace!(
                "sleep block for {:?} with {:?} remaining after",
                microsleep,
                remaining_time
            );
            if let Err(some) = tx.blocking_send(TaskTxMsg::SleepReport(id)) {
                error!("problem sending to App: {:?}", some);
            } else {
                trace!("sent a sleep report")
            }
            sleep(Duration::from_secs(microsleep));
        }
        trace!("done with sum {:?}", sum);
        Some(sum)
    }

    /// Reads all messages. If any are relevant, sends bool so the blocking task can terminate
    #[instrument(skip(tx, rx))]
    fn check_for_term_message(
        id: Id,
        rx: &mut broadcast::Receiver<TaskRxMsg>,
        tx: &mpsc::Sender<TaskTxMsg>,
    ) -> bool {
        loop {
            match rx.try_recv() {
                Ok(TaskRxMsg::PleaseStop(addr_to)) => {
                    if addr_to == id {
                        info!("recieved strong suggestion to terminate, doing so");
                        if let Err(some) = tx.blocking_send(TaskTxMsg::CancelReport(id)) {
                            error!("problem sending cancel report to App {:?}", some)
                        } else {
                            trace!("cancel report sent off to App")
                        }
                        return true;
                    } // Else we keep checking messages
                }
                Ok(TaskRxMsg::EveryoneStopPls) => {
                    info!("recieved terminate-all message, joining the club");
                    return true;
                }
                Err(TryRecvError::Closed) => {
                    info!("recived no message, but App is gone(?). terminating");
                    return true;
                }
                Err(TryRecvError::Lagged(by)) => {
                    warn!("task {id} reports lag of {by} messages")
                    // And keep checking messages
                }
                Err(TryRecvError::Empty) => return false,
            };
        }
    }
}
