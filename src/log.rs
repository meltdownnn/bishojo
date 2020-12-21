use colored::Colorize;
/// A very simple logging level enumeration.
///
/// `Error` is not contained because it should be either `Warning` or `panic!`.
pub enum LoggingLevel {
    StatusReport,
    Message,
    Warning,
}
impl std::fmt::Display for LoggingLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Self::StatusReport => "S",
                Self::Message => "M",
                Self::Warning => "W",
            }
        )
    }
}
/// A very simple logging tool
///
/// This is used instead of traditional logging library because it's easier and more simple.
/// Also, we don't need verbose output logs from libraries that we depends on.
pub struct LoggingClient {
    start_time: std::sync::Arc<std::time::Instant>,
}
impl LoggingClient {
    pub fn new() -> Self {
        Self {
            start_time: std::sync::Arc::new(std::time::Instant::now()),
        }
    }
    pub fn log(&self, level: LoggingLevel, content: &str) {
        let elapsed = self.start_time.elapsed();
        let message = format!(
            "[{}.{}] {}: {}",
            elapsed.as_secs().to_string(),
            elapsed.subsec_micros().to_string(),
            level.to_string(),
            content
        );
        let message = match level {
            LoggingLevel::Warning => message.yellow(),
            LoggingLevel::Message => message.green(),
            LoggingLevel::StatusReport => message.blue(),
        };
        println!("{}", message)
    }
}
