use std::path::Path;

use serde_json::json;

use crate::cli::AddArgs;
use crate::error::CliError;
use crate::index::{AccountParams, Index, validate_name};
use crate::otpauth;
use crate::output::Outcome;
use crate::secret::Secret;
use crate::store::SecretStore;
use crate::totp::Algorithm;

const DEFAULT_DIGITS: u32 = 6;
const DEFAULT_PERIOD: u64 = 30;

pub fn run(
    args: &AddArgs,
    config_dir: &Path,
    store: &dyn SecretStore,
) -> Result<Outcome, CliError> {
    validate_name(&args.name)?;
    let (secret, params) = resolve_secret_and_params(args)?;
    params.validate()?;

    let mut index = Index::load(config_dir)?;
    if index.accounts.contains_key(&args.name) && !args.force {
        return Err(CliError::Usage(format!(
            "account '{}' already exists; pass --force to replace it",
            args.name
        )));
    }

    store.set(&args.name, &secret)?;
    index.accounts.insert(args.name.clone(), params.clone());
    if let Err(save_error) = index.save(config_dir) {
        // Keep the store and the index in agreement: undo the secret we just wrote.
        let _ = store.delete(&args.name);
        return Err(save_error);
    }

    Ok(Outcome {
        human: format!("added {}", args.name),
        json: account_json(&args.name, &params),
    })
}

pub fn account_json(name: &str, params: &AccountParams) -> serde_json::Value {
    let mut value = json!({
        "name": name,
        "digits": params.digits,
        "period": params.period,
        "algorithm": params.algorithm,
    });
    if let Some(issuer) = &params.issuer {
        value["issuer"] = json!(issuer);
    }
    value
}

fn resolve_secret_and_params(args: &AddArgs) -> Result<(Secret, AccountParams), CliError> {
    let parsed_uri = args.uri.as_deref().map(otpauth::parse).transpose()?;
    let (secret, uri_issuer, uri_digits, uri_period, uri_algorithm) = match parsed_uri {
        Some(parsed) => (
            parsed.secret,
            parsed.issuer,
            parsed.digits,
            parsed.period,
            parsed.algorithm,
        ),
        None => (read_secret(args.secret.as_deref())?, None, None, None, None),
    };
    let params = AccountParams {
        issuer: args.issuer.clone().or(uri_issuer),
        digits: args.digits.or(uri_digits).unwrap_or(DEFAULT_DIGITS),
        period: args.period.or(uri_period).unwrap_or(DEFAULT_PERIOD),
        algorithm: args.algorithm.or(uri_algorithm).unwrap_or(Algorithm::Sha1),
    };
    Ok((secret, params))
}

fn read_secret(flag_value: Option<&str>) -> Result<Secret, CliError> {
    match flag_value {
        Some(value) => Secret::parse(value),
        None => {
            let typed = rpassword::prompt_password("Secret (base32): ").map_err(|io_error| {
                CliError::Usage(format!("cannot read secret from terminal: {io_error}"))
            })?;
            Secret::parse(&typed)
        }
    }
}
