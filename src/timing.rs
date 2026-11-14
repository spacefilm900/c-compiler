//! Optional per-phase wall-clock timing for `--time-phases`.
//!
//! Lexing happens lazily inside the preprocessor (each `#include` is lexed
//! when it is reached) and again when preprocessing tokens are converted to
//! parser tokens, so its time is accumulated here and subtracted from the
//! preprocessor's total.

use std::cell::Cell;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

static ENABLED: AtomicBool = AtomicBool::new(false);

/// Turn lexing time accounting on (it costs two clock reads per token).
pub fn enable() {
    ENABLED.store(true, Ordering::Relaxed);
}

thread_local! {
    static LEX: Cell<Duration> = const { Cell::new(Duration::ZERO) };
}

/// Run `f` and add its wall time to the lexing total.
pub(crate) fn lexing<T>(f: impl FnOnce() -> T) -> T {
    if !ENABLED.load(Ordering::Relaxed) {
        return f();
    }
    let start = Instant::now();
    let value = f();
    let spent = start.elapsed();
    LEX.with(|lex| lex.set(lex.get() + spent));
    value
}

/// Take the lexing total and reset it.
pub(crate) fn take_lex() -> Duration {
    LEX.with(|lex| lex.replace(Duration::ZERO))
}

/// Wall time of each frontend phase of one compilation.
#[derive(Clone, Debug, Default)]
pub struct PhaseTimes {
    pub lex: Duration,
    pub preprocess: Duration,
    pub parse: Duration,
    pub sema: Duration,
    pub irgen: Duration,
    pub write: Duration,
}

impl PhaseTimes {
    /// One line of JSON with the phase times in seconds.
    pub fn to_json(&self) -> String {
        format!(
            "{{\"lex\": {:.6}, \"preprocess\": {:.6}, \"parse\": {:.6}, \"sema\": {:.6}, \"irgen\": {:.6}, \"write\": {:.6}}}",
            self.lex.as_secs_f64(),
            self.preprocess.as_secs_f64(),
            self.parse.as_secs_f64(),
            self.sema.as_secs_f64(),
            self.irgen.as_secs_f64(),
            self.write.as_secs_f64(),
        )
    }
}
