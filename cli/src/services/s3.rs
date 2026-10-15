use serde::{Deserialize, Serialize};

use crate::{AwsClient, Result, protocols::rest_xml};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct ListResponse {
    #[serde(default)]
    buckets: Buckets,
}

#[derive(Debug, Default, Deserialize)]
struct Buckets {
    #[serde(rename = "Bucket", default)]
    buckets: Vec<BucketXml>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct BucketXml {
    name: String,
    creation_date: Option<String>,
}

#[derive(Debug, Deserialize)]
struct LocationResponse {
    #[serde(rename = "$text")]
    location: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct Bucket {
    pub name: String,
    pub region: String,
    pub created: Option<String>,
}

pub fn list_buckets(client: &AwsClient) -> Result<Vec<Bucket>> {
    let response: ListResponse = rest_xml::get(
        client,
        "s3",
        "us-east-1",
        "s3.amazonaws.com",
        "/",
        Vec::new(),
    )?;
    let mut output = Vec::new();
    for bucket in response.buckets.buckets {
        let location: LocationResponse = rest_xml::get(
            client,
            "s3",
            "us-east-1",
            "s3.amazonaws.com",
            &format!("/{}", bucket.name),
            vec![("location".into(), String::new())],
        )?;
        output.push(Bucket {
            name: bucket.name,
            region: match location.location.as_deref() {
                None | Some("") => "us-east-1".into(),
                Some("EU") => "eu-west-1".into(),
                Some(value) => value.into(),
            },
            created: bucket
                .creation_date
                .map(|value| value.chars().take(10).collect()),
        });
    }
    output.sort_by(|left, right| {
        left.region
            .cmp(&right.region)
            .then(left.name.cmp(&right.name))
    });
    Ok(output)
}
