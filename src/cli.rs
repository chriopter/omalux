use std::process::ExitCode;

pub(crate) fn run(arguments: impl IntoIterator<Item = String>) -> ExitCode {
    let arguments: Vec<String> = arguments.into_iter().collect();
    match arguments.as_slice() {
        [] => {
            print_help();
            ExitCode::SUCCESS
        }
        [help] if help == "--help" || help == "-h" => {
            print_help();
            ExitCode::SUCCESS
        }
        [version] if version == "--version" || version == "-V" => {
            println!("grainroom {}", env!("CARGO_PKG_VERSION"));
            ExitCode::SUCCESS
        }
        _ => {
            eprintln!(
                "grainroom: no core command was selected; run `grainroom --help` or launch `grainroom-gui`"
            );
            ExitCode::from(2)
        }
    }
}

fn print_help() {
    println!(
        "Grainroom core — Qt-free photo processing\n\n\
Usage:\n  grainroom [OPTIONS]\n\n\
Options:\n  -h, --help       Show this help\n  -V, --version    Show the version\n\n\
The desktop application is installed as `grainroom-gui`. Processing\n\
subcommands will be added to this Qt-free executable in a later package."
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_only_process_level_help_and_version_for_now() {
        assert_eq!(run(Vec::new()), ExitCode::SUCCESS);
        assert_eq!(run(["--help".to_owned()]), ExitCode::SUCCESS);
        assert_eq!(run(["--version".to_owned()]), ExitCode::SUCCESS);
        assert_eq!(run(["export".to_owned()]), ExitCode::from(2));
    }
}
