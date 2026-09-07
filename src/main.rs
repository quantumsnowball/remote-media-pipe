use clap::Parser;

#[derive(Parser)]
#[command(version, about)]
struct Cli {
    name: String,
}

fn main() {
    let args = Cli::parse();
    println!("Hello, {}!", args.name);
}
