use serde::{Deserialize, Serialize};

use crate::{AwsClient, Result, protocols::rest_xml};

const ENDPOINT: &str = "route53.amazonaws.com";
const PREFIX: &str = "/2013-04-01";
const NAMESPACE: &str = "https://route53.amazonaws.com/doc/2013-04-01/";

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct ZonesResponse {
    #[serde(default)]
    hosted_zones: HostedZones,
    next_marker: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
struct HostedZones {
    #[serde(rename = "HostedZone", default)]
    values: Vec<HostedZoneXml>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct HostedZoneXml {
    id: String,
    name: String,
    #[serde(default)]
    config: ZoneConfig,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct ZoneConfig {
    private_zone: Option<bool>,
    comment: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct RecordsResponse {
    #[serde(default)]
    resource_record_sets: RecordSets,
    next_record_name: Option<String>,
    next_record_type: Option<String>,
    next_record_identifier: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
struct RecordSets {
    #[serde(rename = "ResourceRecordSet", default)]
    values: Vec<RecordXml>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct RecordXml {
    name: String,
    r#type: String,
    ttl: Option<u64>,
    alias_target: Option<AliasTarget>,
    #[serde(default)]
    resource_records: ResourceRecords,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct AliasTarget {
    pub hosted_zone_id: Option<String>,
    #[serde(rename = "DNSName")]
    pub dns_name: String,
    pub evaluate_target_health: Option<bool>,
}

#[derive(Debug, Default, Deserialize)]
struct ResourceRecords {
    #[serde(rename = "ResourceRecord", default)]
    values: Vec<ResourceRecord>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct ResourceRecord {
    value: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct HostedZone {
    pub id: String,
    pub name: String,
    pub private: bool,
    pub comment: Option<String>,
    pub records: Vec<Record>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct Record {
    pub name: String,
    pub record_type: String,
    pub ttl: Option<u64>,
    pub values: Vec<String>,
    pub alias_target: Option<AliasTarget>,
}

#[derive(Debug, Serialize)]
#[serde(rename = "CreateHostedZoneRequest", rename_all = "PascalCase")]
struct CreateRequest<'a> {
    #[serde(rename = "@xmlns")]
    xmlns: &'static str,
    name: &'a str,
    caller_reference: &'a str,
    hosted_zone_config: CreateConfig<'a>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "PascalCase")]
struct CreateConfig<'a> {
    comment: &'a str,
    private_zone: bool,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct CreateResponse {
    pub hosted_zone: CreatedZone,
    pub delegation_set: DelegationSet,
    pub change_info: Option<ChangeInfo>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct CreatedZone {
    pub id: String,
    pub name: String,
    pub config: Option<CreatedConfig>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct CreatedConfig {
    pub comment: Option<String>,
    pub private_zone: Option<bool>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct DelegationSet {
    #[serde(default)]
    pub name_servers: NameServers,
}

#[derive(Debug, Default, Deserialize, Serialize)]
pub struct NameServers {
    #[serde(rename = "NameServer", default)]
    pub values: Vec<String>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct ChangeInfo {
    pub id: String,
    pub status: String,
    pub submitted_at: Option<String>,
}

pub fn list_hosted_zones(client: &AwsClient) -> Result<Vec<HostedZone>> {
    let mut marker: Option<String> = None;
    let mut zones = Vec::new();
    loop {
        let query = marker
            .iter()
            .map(|value| ("marker".into(), value.clone()))
            .collect();
        let response: ZonesResponse = rest_xml::get(
            client,
            "route53",
            "us-east-1",
            ENDPOINT,
            &format!("{PREFIX}/hostedzone"),
            query,
        )?;
        zones.extend(response.hosted_zones.values);
        marker = response.next_marker;
        if marker.is_none() {
            break;
        }
    }
    let mut output = Vec::new();
    for zone in zones {
        let id = zone.id.rsplit('/').next().unwrap_or(&zone.id).to_owned();
        let mut name: Option<String> = None;
        let mut record_type: Option<String> = None;
        let mut identifier: Option<String> = None;
        let mut records = Vec::new();
        loop {
            let mut query = Vec::new();
            if let Some(value) = &name {
                query.push(("name".into(), value.clone()));
            }
            if let Some(value) = &record_type {
                query.push(("type".into(), value.clone()));
            }
            if let Some(value) = &identifier {
                query.push(("identifier".into(), value.clone()));
            }
            let response: RecordsResponse = rest_xml::get(
                client,
                "route53",
                "us-east-1",
                ENDPOINT,
                &format!("{PREFIX}/hostedzone/{id}/rrset"),
                query,
            )?;
            records.extend(
                response
                    .resource_record_sets
                    .values
                    .into_iter()
                    .map(|record| Record {
                        name: record.name,
                        record_type: record.r#type,
                        ttl: record.ttl,
                        values: record
                            .resource_records
                            .values
                            .into_iter()
                            .map(|value| value.value)
                            .collect(),
                        alias_target: record.alias_target,
                    }),
            );
            name = response.next_record_name;
            record_type = response.next_record_type;
            identifier = response.next_record_identifier;
            if name.is_none() {
                break;
            }
        }
        output.push(HostedZone {
            id,
            name: zone.name,
            private: zone.config.private_zone.unwrap_or(false),
            comment: zone.config.comment,
            records,
        });
    }
    Ok(output)
}

pub fn create_hosted_zone(
    client: &AwsClient,
    name: &str,
    comment: &str,
    caller_reference: &str,
) -> Result<CreateResponse> {
    rest_xml::post_xml(
        client,
        "route53",
        "us-east-1",
        ENDPOINT,
        &format!("{PREFIX}/hostedzone"),
        &CreateRequest {
            xmlns: NAMESPACE,
            name,
            caller_reference,
            hosted_zone_config: CreateConfig {
                comment,
                private_zone: false,
            },
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_payload_is_typed_and_xml_escaped() {
        let request = CreateRequest {
            xmlns: NAMESPACE,
            name: "example.com",
            caller_reference: "test-1",
            hosted_zone_config: CreateConfig {
                comment: "a & b",
                private_zone: false,
            },
        };
        let xml = quick_xml::se::to_string(&request).unwrap();
        assert!(xml.contains("<Name>example.com</Name>"));
        assert!(xml.contains("a &amp; b"));
        assert!(xml.contains("xmlns=\"https://route53.amazonaws.com/doc/2013-04-01/\""));
    }
}
