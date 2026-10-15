use serde::{Serialize, de::DeserializeOwned};

use crate::{AwsClient, Request, Result};

pub fn call<I: Serialize, O: DeserializeOwned>(
    client: &AwsClient,
    service: &str,
    region: &str,
    endpoint: &str,
    target: &str,
    input: &I,
) -> Result<O> {
    let body = serde_json::to_vec(input)?;
    let mut request = Request::post(service, region, endpoint.into(), body);
    request
        .headers
        .insert("content-type".into(), "application/x-amz-json-1.1".into());
    request.headers.insert("x-amz-target".into(), target.into());
    let response = client.execute(request)?;
    Ok(serde_json::from_slice(&response)?)
}
