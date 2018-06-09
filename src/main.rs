extern crate failure;
extern crate nom;
extern crate serde;
#[macro_use]
extern crate serde_derive;
#[macro_use]
extern crate structopt;
extern crate toml;

mod manifest;
mod spec;

use structopt::StructOpt;

#[derive(StructOpt)]
#[structopt(name = "ipman", about = "An Idris toolchain & package manager")]
struct Ipman {
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
    Install {
        specifier: String,
    },
    #[structopt(name = "uninstall")]
    Uninstall {
        name: String,
    }
}

fn main() {
    let opt = Ipman::from_args();
}
