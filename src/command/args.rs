use clap::{Args, Parser, Subcommand, ValueEnum};
use grainroom::develop::{ParameterOverride, parse_parameter_override};
use std::{path::PathBuf, str::FromStr};

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
    Probe(ProbeArgs),
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
    #[arg(long, value_name = "PATH")]
    pub(crate) input: PathBuf,

    /// Destination image.
    #[arg(short, long, value_name = "PATH")]
    pub(crate) output: PathBuf,

    /// Built-in preset ID.
    #[arg(long, value_name = "ID", conflicts_with = "preset_file")]
    pub(crate) preset: Option<String>,

    /// Complete external preset JSON document.
    #[arg(long, value_name = "PATH", conflicts_with = "preset")]
    pub(crate) preset_file: Option<PathBuf>,

    /// Override one scalar or toggle parameter.
    #[arg(long = "set", value_name = "PARAMETER=VALUE", value_parser = parse_override)]
    pub(crate) overrides: Vec<ParameterOverride>,

    /// Output format; inferred from the destination suffix when omitted.
    #[arg(long, value_enum)]
    pub(crate) format: Option<DevelopFormat>,

    /// JPEG/HEIC quality.
    #[arg(long, default_value_t = 90, value_parser = clap::value_parser!(u8).range(1..=100))]
    pub(crate) quality: u8,

    /// Policy for raster files without a usable color declaration.
    #[arg(long, value_enum, default_value_t = UnprofiledArg::AssumeSrgb)]
    pub(crate) unprofiled: UnprofiledArg,

    /// Metadata retained in the exported file.
    #[arg(long, value_enum, default_value_t = MetadataArg::StripLocation)]
    pub(crate) metadata: MetadataArg,

    /// Straight-alpha handling at the opaque output boundary.
    #[arg(long, default_value = "reject", value_parser = AlphaArg::from_str)]
    pub(crate) alpha: AlphaArg,

    /// Replace an existing regular destination.
    #[arg(long)]
    pub(crate) overwrite: bool,

    /// Emit the final machine-readable result as JSON.
    #[arg(long)]
    pub(crate) json: bool,

    /// Progress stream mode.
    #[arg(long, value_enum, default_value_t = ProgressArg::None)]
    pub(crate) progress: ProgressArg,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub(crate) enum DevelopFormat {
    #[value(alias = "jpg")]
    Jpeg,
    #[value(alias = "heif")]
    Heic,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub(crate) enum UnprofiledArg {
    AssumeSrgb,
    Reject,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub(crate) enum MetadataArg {
    PreserveSafe,
    StripLocation,
    StripAll,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub(crate) enum ProgressArg {
    None,
    Human,
    Json,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum AlphaArg {
    Reject,
    FlattenBlack,
    Flatten([u8; 3]),
}

impl FromStr for AlphaArg {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "reject" => Ok(Self::Reject),
            "flatten-black" => Ok(Self::FlattenBlack),
            _ => {
                let hex = value.strip_prefix("flatten=#").ok_or_else(|| {
                    "alpha must be reject, flatten-black, or flatten=#RRGGBB".to_owned()
                })?;
                if hex.len() != 6 || !hex.bytes().all(|byte| byte.is_ascii_hexdigit()) {
                    return Err("alpha flatten color must be exactly #RRGGBB".to_owned());
                }
                let channel = |start| {
                    u8::from_str_radix(&hex[start..start + 2], 16)
                        .map_err(|_| "alpha flatten color must be exactly #RRGGBB".to_owned())
                };
                Ok(Self::Flatten([channel(0)?, channel(2)?, channel(4)?]))
            }
        }
    }
}

#[derive(Debug, Args)]
pub(crate) struct PresetsArgs {
    #[command(subcommand)]
    pub(crate) command: PresetsCommand,
}

#[derive(Debug, Subcommand)]
pub(crate) enum PresetsCommand {
    /// Emit the built-in preset index as JSON.
    List {
        #[arg(long)]
        json: bool,
    },
    /// Emit one canonical preset document as JSON.
    Show {
        id: String,
        #[arg(long)]
        json: bool,
    },
}

#[derive(Debug, Args)]
pub(crate) struct ParametersArgs {
    #[command(subcommand)]
    pub(crate) command: ParametersCommand,
}

#[derive(Debug, Args)]
pub(crate) struct ProbeArgs {
    #[arg(long)]
    pub(crate) json: bool,
}

#[derive(Debug, Subcommand)]
pub(crate) enum ParametersCommand {
    /// Emit the stable parameter registry as JSON.
    List {
        #[arg(long)]
        json: bool,
    },
}

fn parse_override(value: &str) -> Result<ParameterOverride, String> {
    parse_parameter_override(value).map_err(|error| error.to_string())
}
