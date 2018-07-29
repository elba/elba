extern crate elba;
#[macro_use]
extern crate lazy_static;
#[macro_use]
extern crate slog;

use elba::{
    build::compile,
    build::context::{BuildConfig, CompileMode, Layout},
    util::{config::Config, config::Directories, lock::DirLock},
};
use slog::Logger;
use std::path::PathBuf;

lazy_static! {
    static ref LOGGER: Logger = new_logger();
    static ref CONFIG: Config = config();
}

fn new_logger() -> Logger {
    /*
    let decorator = slog_term::TermDecorator::new().build();
    let drain = slog_term::CompactFormat::new(decorator).build().fuse();
    let drain = slog_async::Async::new(drain).build().fuse();
    */

    // Suppress logging output during tests - we don't need to see it
    Logger::root(slog::Discard, o!())
}

fn config() -> Config {
    let start = env!("CARGO_MANIFEST_DIR");
    let mut cache_path = PathBuf::new();
    cache_path.push(start);
    cache_path.push("tests/data/cache");

    Config {
        directories: Directories {
            cache: cache_path.clone(),
            rest: cache_path.clone(),
        },
        ..Config::default()
    }
}

#[test]
fn compile_single() {
    let start = env!("CARGO_MANIFEST_DIR");
    let p = PathBuf::from(start.clone()).join("tests/data/pkgs/one");
    let lp = DirLock::acquire(&p).unwrap();
    let t = p.join("target");
    let lt = DirLock::acquire(&t).unwrap();
    let layout = Layout::new(lt).unwrap();
    // TODO: build Bin target as well
    let bc = BuildConfig {
        compile_mode: CompileMode::Lib,
    };

    compile(&lp, &layout, &CONFIG, &bc, &LOGGER).unwrap()
}
