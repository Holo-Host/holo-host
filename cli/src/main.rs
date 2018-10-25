#[macro_use]
extern crate structopt;

use std::path::PathBuf;

use structopt::StructOpt;


#[derive(StructOpt, Debug)]
#[structopt(about = "A command line interface for Holo Hosting")]
enum Cli {
    #[structopt(
        name = "init",
        about = "Initialization and installation of Hosting app"
    )]
    Init {
        // #[structopt(long = "port", short = "p", default_value = "3000")]
        // port: u16,
    },

    #[structopt(
        name = "install",
        alias = "i",
        about = "Install an app package"
    )]
    Install {
        #[structopt(parse(from_os_str))]
        dna_package: PathBuf,
    },

    #[structopt(
        name = "uninstall",
        alias = "remove",
        about = "Uninstall an app package (if no agents are hosted?)"
    )]
    Uninstall {
        #[structopt()]
        app_name: String,
    },

    #[structopt(
        name = "start",
        about = "Start the hosting service"
    )]
    Start,

    #[structopt(
        name = "stop",
        about = "Stop the hosting service"
    )]
    Stop,
}

pub fn run() -> Result<String, String>{
    let args = Cli::from_args();
    let result = match args {
        Cli::Init {} => unimplemented!(),
        Cli::Install {dna_package} => unimplemented!(),
        Cli::Uninstall {app_name} => unimplemented!(),
        Cli::Start => unimplemented!(),
        Cli::Stop => unimplemented!(),
    };
    result
}

fn main() {
    if let Err(err) = run() {
        eprintln!("{}", err);
        ::std::process::exit(1);
    }
}
