use serde::de::DeserializeOwned;

use crate::{AwsClient, Request, Result};

pub fn get<T: DeserializeOwned>(
    client: &AwsClient,
    service: &str,
    region: &str,
    endpoint: &str,
    path: &str,
    query: Vec<(String, String)>,
) -> Result<T> {
    let mut request = Request::get(service, region, endpoint.into(), path.into());
    request.query = query;
    let response = client.execute(request)?;
    Ok(quick_xml::de::from_reader(response.as_slice())?)
}

pub fn post_xml<I: serde::Serialize, O: DeserializeOwned>(
    client: &AwsClient,
    service: &str,
    region: &str,
    endpoint: &str,
    path: &str,
    input: &I,
) -> Result<O> {
    let body = quick_xml::se::to_string(input)
        .map_err(|error| crate::Error::InvalidRequest(format!("XML serialization: {error}")))?
        .into_bytes();
    let mut request = Request::post(service, region, endpoint.into(), body);
    request.path = path.into();
    request.retryable = false;
    request
        .headers
        .insert("content-type".into(), "application/xml".into());
    let response = client.execute(request)?;
    Ok(quick_xml::de::from_reader(response.as_slice())?)
}
