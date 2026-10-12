use std::path::Path;

use serde_json::Value;

use crate::commands::add::account_json;
use crate::error::CliError;
use crate::index::Index;
use crate::output::Outcome;

pub fn run(config_dir: &Path) -> Result<Outcome, CliError> {
    let index = Index::load(config_dir)?;
    let names: Vec<&str> = index.accounts.keys().map(String::as_str).collect();
    let json: Vec<Value> = index
        .accounts
        .iter()
        .map(|(name, params)| account_json(name, params))
        .collect();
    Ok(Outcome {
        human: names.join("\n"),
        json: Value::Array(json),
    })
}
