//! Preprocessing round trip: when the compiler accepts an input, the text
//! that `-E` prints for it must compile to the same IR. A difference means
//! the preprocessor's output loses or changes a token (spelling, spacing
//! that keeps tokens apart, string or character literal contents), or the
//! front end treats the same tokens differently in the two runs.

#![no_main]
#[path = "common.rs"]
mod common;

use hcc::Dialect;

libfuzzer_sys::fuzz_target!(|data: &[u8]| {
    let Some(text) = common::source(data) else {
        return;
    };
    let options = common::options();
    for dialect in [Dialect::C11, Dialect::Gnu11] {
        let direct = match hcc::compile_dialect(text, "fuzz.c", &options, dialect) {
            Ok(output) => output.ir,
            Err(diag) => {
                common::check(Err(diag));
                continue;
            }
        };
        let expanded = match hcc::pp::preprocess(text, "fuzz.c", &options) {
            Ok(expanded) => expanded,
            Err(diag) => panic!("compiles, but -E fails: {diag}\n--- source ---\n{text}"),
        };
        match hcc::compile_dialect(&expanded, "fuzz.c", &options, dialect) {
            Ok(output) => assert!(
                output.ir == direct,
                "IR differs after -E ({dialect:?})\n--- source ---\n{text}\n--- -E ---\n{expanded}"
            ),
            Err(diag) => panic!(
                "compiles, but its -E output does not ({dialect:?}): {diag}\n--- source ---\n{text}\n--- -E ---\n{expanded}"
            ),
        }
    }
});
