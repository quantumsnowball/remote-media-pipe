use time::macros::format_description;
use tracing::Level;
use tracing_subscriber::EnvFilter;
use tracing_subscriber::fmt::time::UtcTime;

fn init_debug_logging(env_filter: EnvFilter) {
    let time_format = format_description!("[year]-[month]-[day] [hour]:[minute]:[second]");
    let timer = UtcTime::new(time_format);
    tracing_subscriber::fmt()
        .compact() //
        .with_target(true)
        .with_timer(timer)
        .with_env_filter(env_filter)
        .init();
}

fn init_standard_logging(env_filter: EnvFilter) {
    tracing_subscriber::fmt()
        .compact() //
        .with_target(false)
        .without_time()
        .with_env_filter(env_filter)
        .init();
}

pub fn init_logging() {
    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    let is_debug = env_filter.max_level_hint().map_or(false, |l| l >= Level::DEBUG);
    if is_debug {
        init_debug_logging(env_filter);
    } else {
        init_standard_logging(env_filter);
    }
}
