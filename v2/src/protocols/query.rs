use serde::de::DeserializeOwned;

use crate::{AwsClient, Request, Result};

pub fn encode(operation: &str, version: &str, parameters: &[(String, String)]) -> Vec<u8> {
    let mut fields = vec![
        ("Action".to_owned(), operation.to_owned()),
        ("Version".to_owned(), version.to_owned()),
    ];
    fields.extend_from_slice(parameters);
    crate::sigv4::canonical_query(&fields).into_bytes()
}

pub fn call<T: DeserializeOwned>(
    client: &AwsClient,
    endpoint: &str,
    signing_service: &str,
    region: &str,
    operation: &str,
    version: &str,
    parameters: &[(String, String)],
) -> Result<T> {
    let body = encode(operation, version, parameters);
    let mut request = Request::post(signing_service, region, endpoint.into(), body);
    request.headers.insert(
        "content-type".into(),
        "application/x-www-form-urlencoded; charset=utf-8".into(),
    );
    let response = client.execute(request)?;
    Ok(quick_xml::de::from_reader(response.as_slice())?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manually_constructs_sorted_query_payload() {
        let body = encode(
            "DescribeImages",
            "2016-11-15",
            &[("Owner.1".into(), "self".into())],
        );
        assert_eq!(
            String::from_utf8(body).unwrap(),
            "Action=DescribeImages&Owner.1=self&Version=2016-11-15"
        );
    }
}
