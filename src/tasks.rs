use std::fmt;
use std::time::SystemTime;

#[derive(Debug)]
pub struct Task {
    pub id: u32,
    pub name: String,
    pub status: TaskStatus,
    pub start: SystemTime,
    pub end: Option<SystemTime>,
    pub description: String,
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
