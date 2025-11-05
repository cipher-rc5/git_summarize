// file: src/utils/logging.rs
// description: Tracing subscriber initialization with optional ANSI coloring

use colored::*;
use tracing_subscriber::{EnvFilter, fmt, prelude::*};

pub fn init_logger(colored_output: bool, verbose: bool) {
    let level = if verbose { "debug" } else { "info" };
    let filter = EnvFilter::new(level);

    let fmt_layer = fmt::layer()
        .with_target(false)
        .with_thread_ids(false)
        .with_thread_names(false)
        .with_file(true)
        .with_line_number(true)
        .compact()
        .with_ansi(colored_output);

    tracing_subscriber::registry()
        .with(filter)
        .with(fmt_layer)
        .init();
}

pub fn format_success(msg: &str) -> String {
    format!("{} {}", "✓".green().bold(), msg.green())
}

pub fn format_error(msg: &str) -> String {
    format!("{} {}", "✗".red().bold(), msg.red())
}

pub fn format_warning(msg: &str) -> String {
    format!("{} {}", "⚠".yellow().bold(), msg.yellow())
}

pub fn format_info(msg: &str) -> String {
    format!("{} {}", "ℹ".blue().bold(), msg)
}

pub fn format_step(step: usize, total: usize, msg: &str) -> String {
    format!("{} {}", format!("[{}/{}]", step, total).cyan().bold(), msg)
}
