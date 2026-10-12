use clap::{Parser, Subcommand};

use crate::commands;
use crate::error::CliError;
use crate::index;
use crate::output::Outcome;
use crate::store;
use crate::totp::Algorithm;

#[derive(Parser)]
#[command(
    name = "totp-cli",
    version,
    about = "Generate TOTP codes from secrets kept in the OS credential store",
    propagate_version = true
)]
pub struct Cli {
    /// Print JSON on stdout (and structured JSON errors on stderr)
    #[arg(long, global = true)]
    pub json: bool,
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    /// Store a TOTP secret under a name
    Add(AddArgs),
    /// Print the current code for a stored account
    Code(CodeArgs),
    /// Print the names of the stored accounts (never the secrets)
    List,
    /// Delete a stored account and its secret
    Remove {
        /// Account name given to `add`
        name: String,
    },
}

#[derive(clap::Args)]
pub struct AddArgs {
    /// Local alias, e.g. "netsuite" (letters, digits, '.', '_', '-')
    pub name: String,
    /// Base32 secret; omit both --secret and --uri to be prompted without echo
    #[arg(long, conflicts_with = "uri")]
    pub secret: Option<String>,
    /// otpauth://totp/... URI from the QR code; supplies secret, issuer, digits, period, algorithm
    #[arg(long)]
    pub uri: Option<String>,
    /// Code length: 6, 7 or 8 [default: 6, or the URI's value]
    #[arg(long)]
    pub digits: Option<u32>,
    /// Seconds per code [default: 30, or the URI's value]
    #[arg(long)]
    pub period: Option<u64>,
    /// HMAC algorithm [default: sha1, or the URI's value]
    #[arg(long, ignore_case = true)]
    pub algorithm: Option<Algorithm>,
    /// Display name of the service (informational)
    #[arg(long)]
    pub issuer: Option<String>,
    /// Replace an existing account with the same name
    #[arg(long)]
    pub force: bool,
}

#[derive(clap::Args)]
pub struct CodeArgs {
    /// Account name given to `add`
    pub name: String,
    /// If the current code expires in fewer than this many seconds, wait for the next one
    #[arg(long, default_value_t = 0, value_name = "SECS")]
    pub min_remaining: u64,
    /// Compute the code for this Unix time instead of now (testing / clock-skew debugging)
    #[arg(long, hide = true)]
    pub at: Option<u64>,
}

pub fn cli_main() -> u8 {
    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
        // clap prints its own usage message and uses exit code 2 for bad arguments.
        Err(clap_error) => clap_error.exit(),
    };
    match run_command(&cli.command) {
        Ok(outcome) => {
            outcome.print(cli.json);
            0
        }
        Err(error) => {
            report_error(&error, cli.json);
            error.exit_code()
        }
    }
}

fn run_command(command: &Command) -> Result<Outcome, CliError> {
    let config_dir = index::config_dir()?;
    match command {
        Command::Add(args) => {
            let store = store::open_store(&config_dir)?;
            commands::add::run(args, &config_dir, store.as_ref())
        }
        Command::Code(args) => {
            let store = store::open_store(&config_dir)?;
            commands::code::run(args, &config_dir, store.as_ref())
        }
        Command::List => commands::list::run(&config_dir),
        Command::Remove { name } => {
            let store = store::open_store(&config_dir)?;
            commands::remove::run(name, &config_dir, store.as_ref())
        }
    }
}

fn report_error(error: &CliError, as_json: bool) {
    if as_json {
        eprintln!("{}", error.to_json());
    } else {
        eprintln!("error: {error}");
    }
}
