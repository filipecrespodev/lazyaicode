#![no_main]
libfuzzer_sys::fuzz_target!(|data: &[u8]| lazyaicode_fuzz::term(data));
