#[derive(Debug)]
struct Args {
    help: bool,
    version: bool,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = pico_args::Arguments::from_env();

        // Arguments can be parsed in any order.
    let args = Args {
        // You can use a slice for multiple commands
        help: args.contains(["-h", "--help"]),
        // or just a string for a single one.
        version: args.contains("-V"),
    };

    dbg!(args);

    Ok(())
}
