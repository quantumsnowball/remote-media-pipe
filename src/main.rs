use clap::Parser;

#[derive(Parser)]
#[command(version, about = "A zero-copy remote media streaming proxy")]
struct Cli {
    /// Remote source (e.g. qsc:DLNA/ or gdrive:folder_id)
    remote: String,

    /// Target listen address (e.g. 192.168.1.100:7879)
    target: String,
}

fn main() {
    let args = Cli::parse();

    let remote = &args.remote;
    let target = &args.target;

    println!("Remote : {remote}");
    println!("Target : {target}");
}
