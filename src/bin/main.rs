extern crate failure;
extern crate matic;
#[macro_use]
extern crate structopt;
extern crate toml;

use structopt::StructOpt;

#[derive(StructOpt)]
#[structopt(name = "matic", about = "An Idris package manager")]
struct Matic {
    #[structopt(short = "v", long = "verbose", parse(from_occurrences))]
    verbose: u8,
    #[structopt(subcommand)]
    cmd: Cmd,
}

#[derive(StructOpt)]
enum Cmd {
    #[structopt(name = "build")]
    Build,
    #[structopt(name = "check")]
    Check,
    #[structopt(name = "init")]
    Init {
        #[structopt(long = "lib")]
        lib: bool,
        name: String,
    },
    #[structopt(name = "install")]
    Install { specifier: String },
    #[structopt(name = "uninstall")]
    Uninstall { name: String },
}

fn main() {
    let opt = Matic::from_args();
}
