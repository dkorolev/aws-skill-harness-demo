use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use hmac::{Hmac, Mac};
use percent_encoding::{AsciiSet, NON_ALPHANUMERIC, utf8_percent_encode};
use sha2::{Digest, Sha256};
use zeroize::Zeroizing;

use crate::{
    Credentials,
    error::{Error, Result},
};

type HmacSha256 = Hmac<Sha256>;

const AWS_ENCODE_SET: &AsciiSet = &NON_ALPHANUMERIC
    .remove(b'-')
    .remove(b'_')
    .remove(b'.')
    .remove(b'~');

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Signature {
    pub authorization: String,
    pub amz_date: String,
    pub content_sha256: String,
    pub signed_headers: String,
}

pub fn uri_encode(value: &str) -> String {
    utf8_percent_encode(value, AWS_ENCODE_SET).to_string()
}

pub fn canonical_uri(path: &str) -> String {
    let source = if path.is_empty() { "/" } else { path };
    source
        .split('/')
        .map(uri_encode)
        .collect::<Vec<_>>()
        .join("/")
}

pub fn canonical_query(query: &[(String, String)]) -> String {
    let mut pairs = query
        .iter()
        .map(|(key, value)| (uri_encode(key), uri_encode(value)))
        .collect::<Vec<_>>();
    pairs.sort();
    pairs
        .into_iter()
        .map(|(key, value)| format!("{key}={value}"))
        .collect::<Vec<_>>()
        .join("&")
}

pub fn canonical_headers(headers: &BTreeMap<String, String>) -> (String, String) {
    let normalized = headers
        .iter()
        .map(|(name, value)| {
            (
                name.trim().to_ascii_lowercase(),
                value.split_ascii_whitespace().collect::<Vec<_>>().join(" "),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let names = normalized.keys().cloned().collect::<Vec<_>>().join(";");
    let lines = normalized
        .into_iter()
        .map(|(name, value)| format!("{name}:{value}\n"))
        .collect::<String>();
    (lines, names)
}

#[allow(clippy::too_many_arguments)]
pub fn sign(
    credentials: &Credentials,
    timestamp: DateTime<Utc>,
    method: &str,
    service: &str,
    region: &str,
    path: &str,
    query: &[(String, String)],
    body: &[u8],
    headers: &BTreeMap<String, String>,
) -> Result<Signature> {
    if !headers.keys().any(|key| key.eq_ignore_ascii_case("host")) {
        return Err(Error::InvalidRequest("SigV4 requires a Host header".into()));
    }
    let amz_date = timestamp.format("%Y%m%dT%H%M%SZ").to_string();
    let date = timestamp.format("%Y%m%d").to_string();
    let content_sha256 = hex::encode(Sha256::digest(body));
    let (canonical_header_lines, signed_headers) = canonical_headers(headers);
    let canonical_request = format!(
        "{}\n{}\n{}\n{}\n{}\n{}",
        method.to_ascii_uppercase(),
        canonical_uri(path),
        canonical_query(query),
        canonical_header_lines,
        signed_headers,
        content_sha256,
    );
    let scope = format!("{date}/{region}/{service}/aws4_request");
    let string_to_sign = format!(
        "AWS4-HMAC-SHA256\n{amz_date}\n{scope}\n{}",
        hex::encode(Sha256::digest(canonical_request.as_bytes())),
    );
    let mut root_key = Zeroizing::new(Vec::with_capacity(
        4 + credentials.secret_access_key().len(),
    ));
    root_key.extend_from_slice(b"AWS4");
    root_key.extend_from_slice(credentials.secret_access_key().as_bytes());
    let date_key = Zeroizing::new(hmac(&root_key, date.as_bytes())?);
    let region_key = Zeroizing::new(hmac(&date_key, region.as_bytes())?);
    let service_key = Zeroizing::new(hmac(&region_key, service.as_bytes())?);
    let signing_key = Zeroizing::new(hmac(&service_key, b"aws4_request")?);
    let signature = hex::encode(hmac(&signing_key, string_to_sign.as_bytes())?);
    Ok(Signature {
        authorization: format!(
            "AWS4-HMAC-SHA256 Credential={}/{scope}, SignedHeaders={signed_headers}, Signature={signature}",
            credentials.access_key_id(),
        ),
        amz_date,
        content_sha256,
        signed_headers,
    })
}

fn hmac(key: &[u8], message: &[u8]) -> Result<Vec<u8>> {
    let mut mac = HmacSha256::new_from_slice(key)
        .map_err(|_| Error::InvalidRequest("HMAC rejected signing key".into()))?;
    mac.update(message);
    Ok(mac.finalize().into_bytes().to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn canonicalizes_query_by_encoded_key_and_value() {
        let query = vec![
            ("z".into(), "last".into()),
            ("space".into(), "a b".into()),
            ("a".into(), "/".into()),
        ];
        assert_eq!(canonical_query(&query), "a=%2F&space=a%20b&z=last");
    }

    #[test]
    fn signs_a_fixed_request_deterministically() {
        let credentials = Credentials::new(
            "AKIDEXAMPLE".into(),
            "wJalrXUtnFEMI/K7MDENG+bPxRfiCYEXAMPLEKEY".into(),
            None,
        )
        .unwrap();
        let timestamp = Utc.with_ymd_and_hms(2015, 8, 30, 12, 36, 0).unwrap();
        let headers = BTreeMap::from([
            (
                "content-type".into(),
                "application/x-www-form-urlencoded; charset=utf-8".into(),
            ),
            ("host".into(), "iam.amazonaws.com".into()),
            ("x-amz-date".into(), "20150830T123600Z".into()),
        ]);
        let signature = sign(
            &credentials,
            timestamp,
            "POST",
            "iam",
            "us-east-1",
            "/",
            &[],
            b"Action=ListUsers&Version=2010-05-08",
            &headers,
        )
        .unwrap();
        assert_eq!(signature.amz_date, "20150830T123600Z");
        assert_eq!(signature.signed_headers, "content-type;host;x-amz-date");
        assert!(signature.authorization.starts_with(
            "AWS4-HMAC-SHA256 Credential=AKIDEXAMPLE/20150830/us-east-1/iam/aws4_request"
        ));
        assert_eq!(
            signature.authorization.rsplit('=').next().unwrap().len(),
            64
        );
    }
}
