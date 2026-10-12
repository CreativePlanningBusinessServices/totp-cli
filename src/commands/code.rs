use std::path::Path;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde_json::json;

use crate::cli::CodeArgs;
use crate::error::CliError;
use crate::index::Index;
use crate::output::Outcome;
use crate::store::SecretStore;
use crate::totp::{expires_in, totp};

pub fn run(
    args: &CodeArgs,
    config_dir: &Path,
    store: &dyn SecretStore,
) -> Result<Outcome, CliError> {
    let index = Index::load(config_dir)?;
    let params = index.accounts.get(&args.name).ok_or_else(|| {
        CliError::Usage(format!(
            "no account named '{}'; run `totp-cli list`",
            args.name
        ))
    })?;
    if args.min_remaining >= params.period {
        return Err(CliError::Usage(format!(
            "--min-remaining must be less than the period ({}s)",
            params.period
        )));
    }
    let secret = store.get(&args.name)?.ok_or_else(|| {
        CliError::Store(format!(
            "secret for '{name}' is missing from the credential store; re-add it with `totp-cli add {name} --force`",
            name = args.name
        ))
    })?;

    let now = args.at.map(Ok).unwrap_or_else(current_unix_time)?;
    let time = effective_time(now, params.period, args.min_remaining);
    if time > now && args.at.is_none() {
        std::thread::sleep(Duration::from_secs(time - now));
    }

    let code = totp(
        &secret.key_bytes(),
        time,
        params.period,
        params.digits,
        params.algorithm,
    );
    let remaining = expires_in(time, params.period);
    Ok(Outcome {
        human: code.clone(),
        json: json!({
            "name": args.name,
            "code": code,
            "expires_in": remaining,
            "period": params.period,
        }),
    })
}

/// Returns `now`, or the first second of the next period when the current code would
/// expire in fewer than `min_remaining` seconds.
pub fn effective_time(now: u64, period: u64, min_remaining: u64) -> u64 {
    let remaining = expires_in(now, period);
    if min_remaining > 0 && remaining < min_remaining {
        now + remaining
    } else {
        now
    }
}

fn current_unix_time() -> Result<u64, CliError> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs())
        .map_err(|clock_error| CliError::Io(format!("system clock is before 1970: {clock_error}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn effective_time_only_advances_when_the_code_is_about_to_expire() {
        assert_eq!(effective_time(59, 30, 0), 59);
        assert_eq!(effective_time(59, 30, 5), 60);
        assert_eq!(effective_time(60, 30, 5), 60);
        assert_eq!(effective_time(85, 30, 5), 85);
        assert_eq!(effective_time(86, 30, 5), 90);
    }
}
