#![no_main]
#[path = "common.rs"]
mod common;

use hcc::{Dialect, Stage};

libfuzzer_sys::fuzz_target!(|data: &[u8]| {
    if let Some(text) = common::source(data) {
        let options = common::options();
        for dialect in [Dialect::C11, Dialect::Gnu11] {
            let result = hcc::dump_stage(text, "fuzz.c", &options, dialect, Stage::Ast);
            common::check(result.map(drop));
        }
    }
});
