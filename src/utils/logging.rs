// file: src/utils/logging.rs
// description: tracing subscriber initialization with optional ansi coloring

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
