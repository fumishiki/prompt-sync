use clap::Parser;

fn main() {
    let cli = prompt_sync::Cli::parse();
    let exit_code = match prompt_sync::run(cli) {
        Ok(code) => code,
        Err(err) => {
            eprintln!("error: {err:#}");
            2
        }
    };
    std::process::exit(exit_code);
}
