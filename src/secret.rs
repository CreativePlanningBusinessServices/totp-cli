use data_encoding::BASE32_NOPAD;
use zeroize::Zeroizing;

use crate::error::CliError;

/// A TOTP shared secret as normalised base32. Deliberately has no Debug/Display so it
/// cannot leak through logging or `{:?}` formatting.
pub struct Secret(Zeroizing<String>);

impl Secret {
    pub fn parse(input: &str) -> Result<Secret, CliError> {
        Self::decode(input).map_err(CliError::Usage)
    }

    pub fn from_stored(stored: &str) -> Result<Secret, CliError> {
        Self::decode(stored)
            .map_err(|message| CliError::Store(format!("stored secret is unreadable: {message}")))
    }

    pub fn as_base32(&self) -> &str {
        &self.0
    }

    pub fn key_bytes(&self) -> Zeroizing<Vec<u8>> {
        Zeroizing::new(
            BASE32_NOPAD
                .decode(self.0.as_bytes())
                .expect("Secret only holds base32 that decoded at construction"),
        )
    }

    fn decode(input: &str) -> Result<Secret, String> {
        let normalised: String = input
            .chars()
            .filter(|character| {
                !character.is_whitespace() && *character != '-' && *character != '='
            })
            .map(|character| character.to_ascii_uppercase())
            .collect();
        if normalised.is_empty() {
            return Err("secret is empty".into());
        }
        BASE32_NOPAD
            .decode(normalised.as_bytes())
            .map_err(|_| "secret is not valid base32 (letters A-Z and digits 2-7)".to_string())?;
        Ok(Secret(Zeroizing::new(normalised)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalises_case_whitespace_dashes_and_padding() {
        let secret = Secret::parse(" gezd gnbv-gy3t qojq gezdgnbvgy3tqojq== ").unwrap();
        assert_eq!(secret.as_base32(), "GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ");
        assert_eq!(secret.key_bytes().as_slice(), b"12345678901234567890");
    }

    #[test]
    fn rejects_characters_outside_the_base32_alphabet() {
        // Secret has no Debug on purpose, so unwrap_err() is unavailable here.
        let Err(error) = Secret::parse("GEZDGNBV0189") else {
            panic!("expected a usage error");
        };
        assert!(matches!(error, CliError::Usage(_)));
        assert!(error.to_string().contains("base32"));
    }

    #[test]
    fn rejects_empty_input() {
        assert!(matches!(Secret::parse("   "), Err(CliError::Usage(_))));
    }

    #[test]
    fn stored_values_that_fail_to_decode_are_store_errors() {
        assert!(matches!(
            Secret::from_stored("not!base32"),
            Err(CliError::Store(_))
        ));
    }
}
