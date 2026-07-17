use serde::{Deserialize, Serialize};

use crate::{AwsClient, Result, protocols::rest_xml};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct DistributionList {
    #[serde(default)]
    items: DistributionItems,
    next_marker: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
struct DistributionItems {
    #[serde(rename = "Distribution", default)]
    values: Vec<DistributionXml>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct DistributionXml {
    id: String,
    domain_name: Option<String>,
    status: Option<String>,
    enabled: Option<bool>,
    #[serde(default)]
    aliases: Aliases,
    #[serde(default)]
    origins: Origins,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct Aliases {
    #[serde(default)]
    items: AliasItems,
}

#[derive(Debug, Default, Deserialize)]
struct AliasItems {
    #[serde(rename = "CNAME", default)]
    values: Vec<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct Origins {
    #[serde(default)]
    items: OriginItems,
}

#[derive(Debug, Default, Deserialize)]
struct OriginItems {
    #[serde(rename = "Origin", default)]
    values: Vec<Origin>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct Origin {
    domain_name: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct Distribution {
    pub id: String,
    pub domain_name: Option<String>,
    pub aliases: Vec<String>,
    pub origins: Vec<String>,
    pub status: Option<String>,
    pub enabled: Option<bool>,
}

pub fn list_distributions(client: &AwsClient) -> Result<Vec<Distribution>> {
    let mut marker: Option<String> = None;
    let mut output = Vec::new();
    loop {
        let query = marker
            .iter()
            .map(|value| ("Marker".into(), value.clone()))
            .collect();
        let response: DistributionList = rest_xml::get(
            client,
            "cloudfront",
            "us-east-1",
            "cloudfront.amazonaws.com",
            "/2020-05-31/distribution",
            query,
        )?;
        output.extend(response.items.values.into_iter().map(|distribution| {
            Distribution {
                id: distribution.id,
                domain_name: distribution.domain_name,
                aliases: distribution.aliases.items.values,
                origins: distribution
                    .origins
                    .items
                    .values
                    .into_iter()
                    .map(|origin| origin.domain_name)
                    .collect(),
                status: distribution.status,
                enabled: distribution.enabled,
            }
        }));
        marker = response.next_marker;
        if marker.is_none() {
            break;
        }
    }
    Ok(output)
}
