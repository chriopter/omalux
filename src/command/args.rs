use clap::{Args, Parser, Subcommand, ValueEnum};
use grainroom::develop::{ParameterOverride, parse_parameter_override};
use std::path::PathBuf;

#[derive(Debug, Parser)]
#[command(
    name = "grainroom",
    version,
    about = "Qt-free photo development tools",
    subcommand_required = true,
    arg_required_else_help = true
)]
pub(crate) struct Cli {
    #[command(subcommand)]
    pub(crate) command: Command,
}

#[derive(Debug, Subcommand)]
pub(crate) enum Command {
    /// Launch the packaged desktop sibling.
    Gui(GuiArgs),
    /// Validate a non-interactive development request.
    Develop(DevelopArgs),
    /// Inspect built-in presets.
    Presets(PresetsArgs),
    /// Inspect the stable parameter registry.
    Parameters(ParametersArgs),
    /// Report optional backend capabilities.
    Probe,
}

#[derive(Debug, Args)]
pub(crate) struct GuiArgs {
    /// Open one photo after launching the desktop application.
    #[arg(long, value_name = "PATH")]
    pub(crate) input: Option<PathBuf>,
}

#[derive(Debug, Args)]
pub(crate) struct DevelopArgs {
    /// Source JPEG, PNG, BMP, or camera RAW.
    #[arg(value_name = "INPUT")]
    pub(crate) input: PathBuf,

    /// Destination image.
    #[arg(short, long, value_name = "PATH")]
    pub(crate) output: PathBuf,

    /// Built-in preset ID.
    #[arg(long, value_name = "ID", default_value = "neutral")]
    pub(crate) preset: String,

    /// Override one scalar or toggle parameter.
    #[arg(long = "set", value_name = "PARAMETER=VALUE", value_parser = parse_override)]
    pub(crate) overrides: Vec<ParameterOverride>,

    /// Output format; inferred from the destination suffix when omitted.
    #[arg(long, value_enum)]
    pub(crate) format: Option<DevelopFormat>,

    /// JPEG/HEIC quality.
    #[arg(long, default_value_t = 90, value_parser = clap::value_parser!(u8).range(1..=100))]
    pub(crate) quality: u8,

    /// Replace an existing regular destination.
    #[arg(long)]
    pub(crate) overwrite: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub(crate) enum DevelopFormat {
    #[value(alias = "jpg")]
    Jpeg,
    #[value(alias = "heif")]
    Heic,
}

#[derive(Debug, Args)]
pub(crate) struct PresetsArgs {
    #[command(subcommand)]
    pub(crate) command: PresetsCommand,
}

#[derive(Debug, Subcommand)]
pub(crate) enum PresetsCommand {
    /// Emit the built-in preset index as JSON.
    List,
    /// Emit one canonical preset document as JSON.
    Show { id: String },
}

#[derive(Debug, Args)]
pub(crate) struct ParametersArgs {
    #[command(subcommand)]
    pub(crate) command: ParametersCommand,
}

#[derive(Debug, Subcommand)]
pub(crate) enum ParametersCommand {
    /// Emit the stable parameter registry as JSON.
    List,
}

fn parse_override(value: &str) -> Result<ParameterOverride, String> {
    parse_parameter_override(value).map_err(|error| error.to_string())
}
