use percent_encoding::percent_decode_str;
use url::Url;

use crate::error::CliError;
use crate::secret::Secret;
use crate::totp::Algorithm;

pub struct ParsedUri {
    pub secret: Secret,
    pub issuer: Option<String>,
    pub digits: Option<u32>,
    pub period: Option<u64>,
    pub algorithm: Option<Algorithm>,
}

pub fn parse(uri: &str) -> Result<ParsedUri, CliError> {
    let url = Url::parse(uri)
        .map_err(|parse_error| CliError::Usage(format!("invalid otpauth URI: {parse_error}")))?;
    if url.scheme() != "otpauth" {
        return Err(CliError::Usage(
            "URI must start with otpauth://totp/".into(),
        ));
    }
    match url.host_str() {
        Some("totp") => {}
        Some("hotp") => {
            return Err(CliError::Usage(
                "HOTP (counter-based) URIs are not supported".into(),
            ));
        }
        _ => {
            return Err(CliError::Usage(
                "URI must start with otpauth://totp/".into(),
            ));
        }
    }

    let mut secret = None;
    let mut issuer = None;
    let mut digits = None;
    let mut period = None;
    let mut algorithm = None;
    for (key, value) in url.query_pairs() {
        match key.as_ref() {
            "secret" => secret = Some(Secret::parse(&value)?),
            "issuer" => issuer = Some(value.into_owned()),
            "digits" => digits = Some(parse_number::<u32>("digits", &value)?),
            "period" => period = Some(parse_number::<u64>("period", &value)?),
            "algorithm" => algorithm = Some(value.parse::<Algorithm>()?),
            _ => {}
        }
    }

    let secret =
        secret.ok_or_else(|| CliError::Usage("otpauth URI has no secret parameter".into()))?;
    Ok(ParsedUri {
        secret,
        issuer: issuer.or_else(|| issuer_from_label(url.path())),
        digits,
        period,
        algorithm,
    })
}

/// The label is `/Issuer:account` (percent-encoded); the issuer prefix is optional.
fn issuer_from_label(path: &str) -> Option<String> {
    let label = percent_decode_str(path.trim_start_matches('/')).decode_utf8_lossy();
    label
        .split_once(':')
        .map(|(issuer, _account)| issuer.trim().to_string())
        .filter(|issuer| !issuer.is_empty())
}

fn parse_number<Number: std::str::FromStr>(field: &str, value: &str) -> Result<Number, CliError> {
    value
        .parse::<Number>()
        .map_err(|_| CliError::Usage(format!("otpauth URI has a non-numeric {field} '{value}'")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_every_parameter() {
        let parsed = parse(
            "otpauth://totp/NetSuite:jon%40example.com?secret=GEZDGNBVGY3TQOJQ&issuer=NetSuite&digits=8&period=60&algorithm=SHA256",
        )
        .unwrap();
        assert_eq!(parsed.secret.as_base32(), "GEZDGNBVGY3TQOJQ");
        assert_eq!(parsed.issuer.as_deref(), Some("NetSuite"));
        assert_eq!(parsed.digits, Some(8));
        assert_eq!(parsed.period, Some(60));
        assert_eq!(parsed.algorithm, Some(Algorithm::Sha256));
    }

    #[test]
    fn minimal_uri_leaves_optional_fields_none() {
        let parsed = parse("otpauth://totp/jon?secret=GEZDGNBVGY3TQOJQ").unwrap();
        assert_eq!(parsed.issuer, None);
        assert_eq!(parsed.digits, None);
        assert_eq!(parsed.period, None);
        assert_eq!(parsed.algorithm, None);
    }

    #[test]
    fn issuer_falls_back_to_the_percent_encoded_label_prefix() {
        let parsed =
            parse("otpauth://totp/Creative%20Planning:jon?secret=GEZDGNBVGY3TQOJQ").unwrap();
        assert_eq!(parsed.issuer.as_deref(), Some("Creative Planning"));
    }

    #[test]
    fn rejects_hotp_other_schemes_and_missing_secret() {
        assert!(matches!(
            parse("otpauth://hotp/x?secret=GEZDGNBVGY3TQOJQ&counter=0"),
            Err(CliError::Usage(_))
        ));
        assert!(matches!(
            parse("https://example.com/?secret=GEZDGNBVGY3TQOJQ"),
            Err(CliError::Usage(_))
        ));
        assert!(matches!(
            parse("otpauth://totp/x?issuer=Y"),
            Err(CliError::Usage(_))
        ));
        assert!(matches!(parse("not a uri"), Err(CliError::Usage(_))));
    }

    #[test]
    fn rejects_non_numeric_digits_and_period() {
        assert!(matches!(
            parse("otpauth://totp/x?secret=GEZDGNBVGY3TQOJQ&digits=six"),
            Err(CliError::Usage(_))
        ));
        assert!(matches!(
            parse("otpauth://totp/x?secret=GEZDGNBVGY3TQOJQ&period=-1"),
            Err(CliError::Usage(_))
        ));
    }
}
