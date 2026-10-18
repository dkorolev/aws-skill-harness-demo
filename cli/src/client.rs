use std::{collections::BTreeMap, sync::Arc, thread, time::Duration};

use chrono::Utc;
use reqwest::{
    Method,
    blocking::Client,
    header::{HeaderMap, HeaderName, HeaderValue},
};
use sha2::Digest;

use crate::{
    Credentials,
    error::{Error, Result},
    sigv4,
};

#[derive(Clone)]
pub struct Request {
    pub method: Method,
    pub service: String,
    pub region: String,
    pub host: String,
    pub path: String,
    pub query: Vec<(String, String)>,
    pub headers: BTreeMap<String, String>,
    pub body: Vec<u8>,
    /// Whether a lost or throttled attempt may be re-sent. Reads are
    /// idempotent; Route 53 writes are not (a DELETE that succeeded but whose
    /// response was lost would fail on replay), so writers set this to false.
    pub retryable: bool,
}

impl Request {
    pub fn post(service: &str, region: &str, host: String, body: Vec<u8>) -> Self {
        Self {
            method: Method::POST,
            service: service.into(),
            region: region.into(),
            host,
            path: "/".into(),
            query: Vec::new(),
            headers: BTreeMap::new(),
            body,
            retryable: true,
        }
    }

    pub fn get(service: &str, region: &str, host: String, path: String) -> Self {
        Self {
            method: Method::GET,
            service: service.into(),
            region: region.into(),
            host,
            path,
            query: Vec::new(),
            headers: BTreeMap::new(),
            body: Vec::new(),
            retryable: true,
        }
    }
}

impl std::fmt::Debug for Request {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let headers = self
            .headers
            .keys()
            .map(|name| {
                if name.eq_ignore_ascii_case("authorization")
                    || name.eq_ignore_ascii_case("x-amz-security-token")
                {
                    (name.as_str(), "[redacted]")
                } else {
                    (name.as_str(), self.headers[name].as_str())
                }
            })
            .collect::<BTreeMap<_, _>>();
        formatter
            .debug_struct("Request")
            .field("method", &self.method)
            .field("service", &self.service)
            .field("region", &self.region)
            .field("host", &self.host)
            .field("path", &self.path)
            .field("query", &self.query)
            .field("headers", &headers)
            .field("body_len", &self.body.len())
            .field("retryable", &self.retryable)
            .finish()
    }
}

pub struct AwsClient {
    credentials: Arc<Credentials>,
    http: Client,
}

impl AwsClient {
    pub fn new(credentials: Credentials) -> Result<Self> {
        let http = Client::builder()
            .https_only(true)
            .redirect(reqwest::redirect::Policy::none())
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(120))
            .user_agent(concat!("awsw/", env!("CARGO_PKG_VERSION")))
            .build()?;
        Ok(Self {
            credentials: Arc::new(credentials),
            http,
        })
    }

    pub fn execute(&self, mut request: Request) -> Result<Vec<u8>> {
        request.headers.insert("host".into(), request.host.clone());
        let timestamp = Utc::now();
        let amz_date = timestamp.format("%Y%m%dT%H%M%SZ").to_string();
        request.headers.insert("x-amz-date".into(), amz_date);
        if let Some(token) = self.credentials.session_token() {
            request
                .headers
                .insert("x-amz-security-token".into(), token.into());
        }
        if request.service == "s3" {
            let content_hash = hex::encode(sha2::Sha256::digest(&request.body));
            request
                .headers
                .insert("x-amz-content-sha256".into(), content_hash);
        }
        let signature = sigv4::sign(
            &self.credentials,
            timestamp,
            request.method.as_str(),
            &request.service,
            &request.region,
            &request.path,
            &request.query,
            &request.body,
            &request.headers,
        )?;
        request
            .headers
            .insert("authorization".into(), signature.authorization);

        let query = sigv4::canonical_query(&request.query);
        let mut url = format!(
            "https://{}{}",
            request.host,
            sigv4::canonical_uri(&request.path)
        );
        if !query.is_empty() {
            url.push('?');
            url.push_str(&query);
        }
        let mut headers = HeaderMap::new();
        for (name, value) in &request.headers {
            headers.insert(
                HeaderName::from_bytes(name.as_bytes())
                    .map_err(|error| Error::InvalidRequest(error.to_string()))?,
                HeaderValue::from_str(value)
                    .map_err(|error| Error::InvalidRequest(error.to_string()))?,
            );
        }
        let attempts = if request.retryable { ATTEMPTS } else { 1 };
        for attempt in 0..attempts {
            let result = self
                .http
                .request(request.method.clone(), &url)
                .headers(headers.clone())
                .body(request.body.clone())
                .send();
            let response = match result {
                Ok(response) => response,
                Err(_error) if attempt + 1 < attempts => {
                    thread::sleep(backoff(attempt));
                    continue;
                }
                Err(error) => return Err(error.into()),
            };
            let status = response.status();
            let request_id = response
                .headers()
                .get("x-amzn-requestid")
                .or_else(|| response.headers().get("x-amz-request-id"))
                .and_then(|value| value.to_str().ok())
                .map(str::to_owned);
            let body = response.bytes()?.to_vec();
            if status.is_success() {
                return Ok(body);
            }
            let error = parse_error(status.as_u16(), request_id, &body);
            if attempt + 1 < attempts && is_retryable(&error) {
                thread::sleep(backoff(attempt));
                continue;
            }
            return Err(error);
        }
        Err(Error::InvalidRequest("request retry loop exhausted".into()))
    }
}

const ATTEMPTS: usize = 5;

/// Exponential backoff with full jitter: 200 ms, 400 ms, 800 ms, 1.6 s caps.
fn backoff(attempt: usize) -> Duration {
    let cap_ms = 200u64 << attempt.min(4);
    let nanos = u64::from(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.subsec_nanos())
            .unwrap_or(0),
    );
    Duration::from_millis(nanos % (cap_ms + 1))
}

/// AWS signals throttling inconsistently: EC2 uses 503 RequestLimitExceeded,
/// IAM/STS use HTTP 400 Throttling, JSON services use 400 ThrottlingException.
/// Status alone is therefore not enough; the parsed code must be consulted.
fn is_retryable(error: &Error) -> bool {
    match error {
        Error::Aws { status, code, .. } => {
            matches!(status, 429 | 500 | 502 | 503 | 504)
                || matches!(
                    code.as_str(),
                    "Throttling"
                        | "ThrottlingException"
                        | "ThrottledException"
                        | "RequestThrottled"
                        | "RequestThrottledException"
                        | "RequestLimitExceeded"
                        | "TooManyRequestsException"
                        | "ProvisionedThroughputExceededException"
                        | "LimitExceededException"
                        | "SlowDown"
                        | "PriorRequestNotComplete"
                )
        }
        _ => false,
    }
}

fn parse_error(status: u16, request_id: Option<String>, body: &[u8]) -> Error {
    if let Ok(value) = serde_json::from_slice::<serde_json::Value>(body) {
        let code = value
            .get("__type")
            .or_else(|| value.get("code"))
            .and_then(|value| value.as_str())
            .unwrap_or("HTTPError")
            .rsplit('#')
            .next()
            .unwrap_or("HTTPError");
        let message = value
            .get("message")
            .or_else(|| value.get("Message"))
            .and_then(|value| value.as_str())
            .unwrap_or("AWS request failed");
        return Error::aws(status, code, message, request_id);
    }
    if let Ok(value) = quick_xml::de::from_reader::<_, XmlQueryError>(body) {
        let request_id = request_id.or(value.request_id);
        return Error::aws(
            status,
            value.errors.error.code,
            value.errors.error.message,
            request_id,
        );
    }
    if let Ok(value) = quick_xml::de::from_reader::<_, XmlEnvelopeError>(body) {
        let request_id = request_id.or(value.request_id);
        return Error::aws(status, value.error.code, value.error.message, request_id);
    }
    if let Ok(value) = quick_xml::de::from_reader::<_, XmlDirectError>(body) {
        let request_id = request_id.or(value.request_id);
        return Error::aws(status, value.code, value.message, request_id);
    }
    let text = String::from_utf8_lossy(body);
    Error::aws(status, "HTTPError", text, request_id)
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "PascalCase")]
struct XmlQueryError {
    errors: XmlErrors,
    #[serde(rename = "RequestID", alias = "RequestId")]
    request_id: Option<String>,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "PascalCase")]
struct XmlErrors {
    error: XmlErrorValue,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "PascalCase")]
struct XmlEnvelopeError {
    error: XmlErrorValue,
    request_id: Option<String>,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "PascalCase")]
struct XmlDirectError {
    code: String,
    message: String,
    request_id: Option<String>,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "PascalCase")]
struct XmlErrorValue {
    code: String,
    message: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_query_protocol_error_without_exposing_request() {
        let body = br#"<Response><Errors><Error><Code>AccessDenied</Code><Message>no</Message></Error></Errors><RequestID>req-1</RequestID></Response>"#;
        let error = parse_error(403, None, body).to_string();
        assert_eq!(error, "AWS 403 AccessDenied: no (request req-1)");
    }

    #[test]
    fn extracts_rest_xml_error() {
        let body = br#"<Error><Code>NoSuchBucket</Code><Message>gone</Message><RequestId>req-2</RequestId></Error>"#;
        let error = parse_error(404, None, body).to_string();
        assert_eq!(error, "AWS 404 NoSuchBucket: gone (request req-2)");
    }

    #[test]
    fn iam_throttling_at_http_400_is_retryable() {
        let body = br#"<ErrorResponse><Error><Type>Sender</Type><Code>Throttling</Code><Message>Rate exceeded</Message></Error><RequestId>req-4</RequestId></ErrorResponse>"#;
        assert!(is_retryable(&parse_error(400, None, body)));
        let json = br#"{"__type":"ThrottlingException","message":"Rate exceeded"}"#;
        assert!(is_retryable(&parse_error(400, None, json)));
        let denied = br#"<ErrorResponse><Error><Code>AccessDenied</Code><Message>no</Message></Error></ErrorResponse>"#;
        assert!(!is_retryable(&parse_error(403, None, denied)));
        assert!(is_retryable(&parse_error(503, None, b"")));
    }

    #[test]
    fn request_debug_redacts_signing_material() {
        let mut request = Request::get("s3", "us-east-1", "s3.amazonaws.com".into(), "/".into());
        request
            .headers
            .insert("authorization".into(), "AWS4-HMAC-SHA256 secret".into());
        request
            .headers
            .insert("x-amz-security-token".into(), "TOKEN123".into());
        request
            .headers
            .insert("x-amz-date".into(), "20260101T000000Z".into());
        let debug = format!("{request:?}");
        assert!(!debug.contains("secret"), "{debug}");
        assert!(!debug.contains("TOKEN123"), "{debug}");
        assert!(debug.contains("20260101T000000Z"), "{debug}");
    }

    #[test]
    fn extracts_iam_error_envelope() {
        let body = br#"<ErrorResponse><Error><Type>Sender</Type><Code>NoSuchEntity</Code><Message>missing</Message></Error><RequestId>req-3</RequestId></ErrorResponse>"#;
        let error = parse_error(404, None, body).to_string();
        assert_eq!(error, "AWS 404 NoSuchEntity: missing (request req-3)");
    }
}
