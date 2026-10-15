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
    Ok(serde_json::from_slice(&response)?)
}
