use std::str::FromStr;

use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use sha1::Sha1;
use sha2::{Sha256, Sha512};

use crate::error::CliError;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, clap::ValueEnum)]
#[serde(rename_all = "lowercase")]
#[value(rename_all = "lowercase")]
pub enum Algorithm {
    Sha1,
    Sha256,
    Sha512,
}

pub fn totp(key: &[u8], unix_time: u64, period: u64, digits: u32, algorithm: Algorithm) -> String {
    hotp(key, unix_time / period, digits, algorithm)
}

pub fn expires_in(unix_time: u64, period: u64) -> u64 {
    period - (unix_time % period)
}

pub fn hotp(key: &[u8], counter: u64, digits: u32, algorithm: Algorithm) -> String {
    let digest = hmac_digest(key, &counter.to_be_bytes(), algorithm);
    // RFC 4226 §5.3 dynamic truncation: the low nibble of the last byte picks a 4-byte window.
    let offset = usize::from(digest[digest.len() - 1] & 0x0f);
    let window = [
        digest[offset] & 0x7f,
        digest[offset + 1],
        digest[offset + 2],
        digest[offset + 3],
    ];
    let truncated = u32::from_be_bytes(window);
    let code = truncated % 10u32.pow(digits);
    format!("{code:0width$}", width = digits as usize)
}

fn hmac_digest(key: &[u8], message: &[u8], algorithm: Algorithm) -> Vec<u8> {
    match algorithm {
        Algorithm::Sha1 => {
            let mut mac = Hmac::<Sha1>::new_from_slice(key).expect("HMAC accepts any key length");
            mac.update(message);
            mac.finalize().into_bytes().to_vec()
        }
        Algorithm::Sha256 => {
            let mut mac = Hmac::<Sha256>::new_from_slice(key).expect("HMAC accepts any key length");
            mac.update(message);
            mac.finalize().into_bytes().to_vec()
        }
        Algorithm::Sha512 => {
            let mut mac = Hmac::<Sha512>::new_from_slice(key).expect("HMAC accepts any key length");
            mac.update(message);
            mac.finalize().into_bytes().to_vec()
        }
    }
}

impl FromStr for Algorithm {
    type Err = CliError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        match text.to_ascii_lowercase().replace('-', "").as_str() {
            "sha1" => Ok(Algorithm::Sha1),
            "sha256" => Ok(Algorithm::Sha256),
            "sha512" => Ok(Algorithm::Sha512),
            other => Err(CliError::Usage(format!(
                "unsupported algorithm '{other}'; use sha1, sha256 or sha512"
            ))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SHA1_KEY: &[u8] = b"12345678901234567890";
    const SHA256_KEY: &[u8] = b"12345678901234567890123456789012";
    const SHA512_KEY: &[u8] = b"1234567890123456789012345678901234567890123456789012345678901234";

    #[test]
    fn hotp_matches_rfc4226_appendix_d() {
        let expected = [
            "755224", "287082", "359152", "969429", "338314", "254676", "287922", "162583",
            "399871", "520489",
        ];
        for (counter, code) in expected.iter().enumerate() {
            assert_eq!(hotp(SHA1_KEY, counter as u64, 6, Algorithm::Sha1), *code);
        }
    }

    #[test]
    fn totp_matches_rfc6238_appendix_b() {
        let vectors: [(u64, &str, &str, &str); 6] = [
            (59, "94287082", "46119246", "90693936"),
            (1_111_111_109, "07081804", "68084774", "25091201"),
            (1_111_111_111, "14050471", "67062674", "99943326"),
            (1_234_567_890, "89005924", "91819424", "93441116"),
            (2_000_000_000, "69279037", "90698825", "38618901"),
            (20_000_000_000, "65353130", "77737706", "47863826"),
        ];
        for (time, sha1_code, sha256_code, sha512_code) in vectors {
            assert_eq!(totp(SHA1_KEY, time, 30, 8, Algorithm::Sha1), sha1_code);
            assert_eq!(
                totp(SHA256_KEY, time, 30, 8, Algorithm::Sha256),
                sha256_code
            );
            assert_eq!(
                totp(SHA512_KEY, time, 30, 8, Algorithm::Sha512),
                sha512_code
            );
        }
    }

    #[test]
    fn six_digit_codes_are_zero_padded() {
        assert_eq!(
            totp(SHA1_KEY, 1_111_111_109, 30, 6, Algorithm::Sha1),
            "081804"
        );
    }

    #[test]
    fn expires_in_counts_down_to_the_period_boundary() {
        assert_eq!(expires_in(59, 30), 1);
        assert_eq!(expires_in(60, 30), 30);
        assert_eq!(expires_in(75, 30), 15);
    }

    #[test]
    fn algorithm_parses_common_spellings() {
        assert_eq!("sha1".parse::<Algorithm>().unwrap(), Algorithm::Sha1);
        assert_eq!("SHA-256".parse::<Algorithm>().unwrap(), Algorithm::Sha256);
        assert_eq!("Sha512".parse::<Algorithm>().unwrap(), Algorithm::Sha512);
        assert!("md5".parse::<Algorithm>().is_err());
    }
}
