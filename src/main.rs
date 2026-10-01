mod daemon;
mod model;
mod protocol;
mod theme;
mod ui;
mod wire;

fn main() -> anyhow::Result<()> {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    anyhow::ensure!(
        arguments.len() <= 1,
        "Expected at most one argument. Use --help for usage."
    );
    match arguments.first().map(String::as_str) {
        Some("--daemon") => daemon::run(),
        Some("--status") => {
            println!("{}", serde_json::to_string_pretty(&daemon::status()?)?);
            Ok(())
        }
        Some("--check-config") => {
            let config = model::Profiles::load(&model::config_path())?;
            println!(
                "{} bindings in {}",
                config.active().bindings.len(),
                model::config_path().display()
            );
            Ok(())
        }
        Some("--help") => {
            println!(
                "Codex Micro\n\nOpen the app with no arguments.\n  --daemon         Run the background device service\n  --status         Read the live device status\n  --check-config   Validate saved bindings"
            );
            Ok(())
        }
        Some(argument) => anyhow::bail!("Unknown argument: {argument}"),
        None => ui::run(),
    }
}
