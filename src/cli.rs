use clap::{Args, Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(name = "agyline")]
#[command(version, about = "Antigravity CLI status line")]
pub struct Cli {
    /// Enter TUI configuration mode
    #[arg(short = 'c', long = "config")]
    pub config: bool,

    /// Save a sanitized diagnostic payload while rendering stdin (opt-in)
    #[arg(long, value_name = "FILE", conflicts_with = "config")]
    pub capture: Option<std::path::PathBuf>,

    /// Render task lines for `subagentStatusLine` (also detected from the
    /// input)
    #[arg(long, hide = true)]
    pub subagent: bool,

    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Render built-in sample sessions (or a captured JSON file) with your config
    Preview(PreviewArgs),
    /// Install a stable executable and reversibly connect AGY settings
    Install(IntegrationArgs),
    /// Show integration ownership, paths and restoration record
    Status(IntegrationArgs),
    /// Restore only the statusLine captured during installation
    Uninstall(IntegrationArgs),
    /// Refresh the cached git status of a directory (started in the
    /// background by renders)
    #[command(hide = true)]
    RefreshGit { dir: std::path::PathBuf },
    /// Compatibility no-op; agyline does not query upstream release servers
    #[command(hide = true)]
    UpdateCheck,
    /// Run a `cmd` item's command and cache its output (started in the
    /// background by renders)
    #[command(hide = true)]
    RefreshCmd {
        #[arg(long)]
        cwd: Option<std::path::PathBuf>,
        #[arg(long)]
        session: Option<String>,
        #[arg(long)]
        timeout_ms: u64,
        #[arg(last = true)]
        run: String,
    },
}

#[derive(Args, Debug, Default)]
pub struct IntegrationArgs {
    /// Host settings.json (default: AGY_CONFIG_DIR/settings.json)
    #[arg(long)]
    pub settings: Option<std::path::PathBuf>,
    /// Stable tool directory (default: AGYLINE_HOME)
    #[arg(long)]
    pub home: Option<std::path::PathBuf>,
}

impl IntegrationArgs {
    pub fn installation(self) -> crate::integration::Installation {
        crate::integration::Installation::new(
            self.settings
                .unwrap_or_else(crate::cc_settings::Settings::user_path),
            self.home.unwrap_or_else(crate::paths::home),
        )
    }
}

#[derive(Args, Debug)]
pub struct PreviewArgs {
    /// Scenario name, or a path to a status line JSON file
    #[arg(short, long, default_value = "antigravity")]
    pub scenario: String,

    /// Render every built-in scenario
    #[arg(long)]
    pub all: bool,

    /// List the built-in scenarios
    #[arg(long)]
    pub list: bool,

    /// Render the sample background tasks with your subagent line
    #[arg(long)]
    pub subagent: bool,

    /// Columns available (default: $COLUMNS, else 100)
    #[arg(short, long)]
    pub width: Option<usize>,

    /// Color depth: auto, truecolor, 256, 16 or none
    #[arg(long, default_value = "auto")]
    pub color: String,
}

impl Cli {
    pub fn parse_args() -> Self {
        Self::parse()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preview_defaults_to_the_native_agy_scenario() {
        let cli = Cli::try_parse_from(["agyline", "preview"]).unwrap();
        let Some(Command::Preview(arguments)) = cli.command else {
            panic!("expected preview");
        };
        assert_eq!(arguments.scenario, "antigravity");
    }
}
