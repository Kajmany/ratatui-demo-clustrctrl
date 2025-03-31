use crate::task_picker::CandidateTask;
use chrono::{DateTime, Local, TimeZone};
use std::fmt;
use std::thread::sleep;
use std::time::{Duration, SystemTime};
use tokio::task::{self, JoinHandle};
use tracing::trace;

#[derive(Debug)]
pub struct Task {
    pub id: u32,
    pub name: &'static str,
    pub status: TaskStatus,
    pub start: DateTime<Local>,
    pub end: Option<DateTime<Local>>,
    pub description: &'static str,
    pub handle: JoinHandle<i128>,
}

#[derive(Debug)]
pub enum TaskStatus {
    Running,
    Sleeping,
    OnStrike,
    KnownUnknown,
}

impl fmt::Display for TaskStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TaskStatus::Running => write!(f, "Running"),
            TaskStatus::Sleeping => write!(f, "Sleeping"),
            TaskStatus::OnStrike => write!(f, "Strike!"),
            TaskStatus::KnownUnknown => write!(f, "???"),
        }
    }
}

impl Task {
    pub fn new(ct: &CandidateTask) -> Self {
        let handle = task::spawn_blocking(move || {
            // The game was rigged all along
            let mut time_to_sleep = rand::random_range(2..60);
            trace!("total sleep: {:?}", time_to_sleep);
            let mut sum: i128 = 0;
            while time_to_sleep > 0 {
                // Do some really hecking important work
                trace!("sum: {:?}", sum);
                sum = rand::random_iter::<i32>()
                    .take(111333777)
                    .fold(sum, |acc, num| acc + ((num as i128 % 500).abs()));
                let microsleep = rand::random_range(1..(time_to_sleep + 1));
                time_to_sleep -= microsleep;
                trace!(
                    "sleep block for {:?} with {:?} remaining after",
                    microsleep,
                    time_to_sleep
                );
                sleep(Duration::from_secs(microsleep));
            }
            trace!("done with sum {:?}", sum);
            sum
        });
        let start = Local::now();

        //TODO: This
        Self {
            id: 0,
            name: ct.name,
            status: TaskStatus::KnownUnknown,
            start,
            end: None,
            description: ct.description,
            handle,
        }
    }
}
