use std::{collections::BTreeMap, sync::Arc, time::Duration};

use chrono::Utc;
use reqwest::{
    Method,
    blocking::Client,
    header::{HeaderMap, HeaderName, HeaderValue},
};

use crate::{
    Credentials,
    error::{Error, Result},
    sigv4,
};

#[derive(Debug, Clone)]
pub struct Request {
    pub method: Method,
    pub service: String,
    pub region: String,
    pub host: String,
    pub path: String,
    pub query: Vec<(String, String)>,
    pub headers: BTreeMap<String, String>,
    pub body: Vec<u8>,
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
        }
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
            .timeout(Duration::from_secs(120))
            .user_agent(concat!("awsw-v2/", env!("CARGO_PKG_VERSION")))
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
        let response = self
            .http
            .request(request.method, url)
            .headers(headers)
            .body(request.body)
            .send()?;
        let status = response.status();
        let request_id = response
            .headers()
            .get("x-amzn-requestid")
            .or_else(|| response.headers().get("x-amz-request-id"))
            .and_then(|value| value.to_str().ok())
            .map(str::to_owned);
        let body = response.bytes()?.to_vec();
        if !status.is_success() {
            return Err(parse_error(status.as_u16(), request_id, &body));
        }
        Ok(body)
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
    let text = String::from_utf8_lossy(body);
    Error::aws(status, "HTTPError", text, request_id)
}

use sha2::Digest;
