use clap::Parser;

mod args;
use args::{Args, Mode};

fn main() {
    let args = Args::parse();

    match args.mode {
        Mode::Config => {}
        Mode::Run => {}
    }
}
