#![no_main]
#[path = "common.rs"]
mod common;

libfuzzer_sys::fuzz_target!(|data: &[u8]| {
    if let Some(text) = common::source(data) {
        common::check(hcc::lexer::lex(text, "fuzz.c").map(drop));
    }
});
