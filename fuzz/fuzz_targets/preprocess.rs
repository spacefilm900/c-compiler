#![no_main]
#[path = "common.rs"]
mod common;

libfuzzer_sys::fuzz_target!(|data: &[u8]| {
    if let Some(text) = common::source(data) {
        let options = common::options();
        common::check(hcc::pp::preprocess(text, "fuzz.c", &options).map(drop));
    }
});
