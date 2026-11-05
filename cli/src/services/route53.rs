use serde::{Deserialize, Serialize};

use crate::{AwsClient, Error, Result, protocols::rest_xml};

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
    #[serde(rename = "TTL")]
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

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct ChangeResponse {
    pub change_info: ChangeInfo,
}

#[derive(Debug, Clone, Copy)]
pub enum RecordChangeAction {
    Upsert,
    Delete,
}

impl RecordChangeAction {
    fn as_str(self) -> &'static str {
        match self {
            Self::Upsert => "UPSERT",
            Self::Delete => "DELETE",
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename = "ChangeResourceRecordSetsRequest", rename_all = "PascalCase")]
struct ChangeRecordsRequest<'a> {
    #[serde(rename = "@xmlns")]
    xmlns: &'static str,
    change_batch: ChangeBatch<'a>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "PascalCase")]
struct ChangeBatch<'a> {
    #[serde(skip_serializing_if = "str::is_empty")]
    comment: &'a str,
    changes: Changes<'a>,
}

#[derive(Debug, Serialize)]
struct Changes<'a> {
    #[serde(rename = "Change")]
    values: Vec<RecordChange<'a>>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "PascalCase")]
struct RecordChange<'a> {
    action: &'static str,
    resource_record_set: RecordSetChange<'a>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "PascalCase")]
struct RecordSetChange<'a> {
    name: &'a str,
    r#type: &'static str,
    #[serde(rename = "TTL")]
    ttl: u64,
    resource_records: ChangeResourceRecords,
}

#[derive(Debug, Serialize)]
struct ChangeResourceRecords {
    #[serde(rename = "ResourceRecord")]
    values: Vec<ChangeResourceRecord>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "PascalCase")]
struct ChangeResourceRecord {
    value: String,
}

fn list_zone_summaries(client: &AwsClient) -> Result<Vec<HostedZoneXml>> {
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
    Ok(zones)
}

pub fn list_hosted_zones(client: &AwsClient) -> Result<Vec<HostedZone>> {
    let mut output = Vec::new();
    for zone in list_zone_summaries(client)? {
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

/// Record types `awsw` can write. Alias records, routing policies, and the
/// remaining types are deliberately out of scope: they are rarer and easier to
/// get wrong from a command line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecordType {
    A,
    Aaaa,
    Cname,
    Txt,
}

impl RecordType {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::A => "A",
            Self::Aaaa => "AAAA",
            Self::Cname => "CNAME",
            Self::Txt => "TXT",
        }
    }

    /// TXT records are mostly short-lived ACME proofs; everything else is a
    /// longer-lived pointer that still has to be cheap to move.
    pub fn default_ttl(self) -> u64 {
        match self {
            Self::Txt => 60,
            _ => 300,
        }
    }
}

impl std::str::FromStr for RecordType {
    type Err = String;

    fn from_str(value: &str) -> std::result::Result<Self, Self::Err> {
        match value.to_ascii_uppercase().as_str() {
            "A" => Ok(Self::A),
            "AAAA" => Ok(Self::Aaaa),
            "CNAME" => Ok(Self::Cname),
            "TXT" => Ok(Self::Txt),
            _ => Err(format!(
                "unsupported record type `{value}` (expected A, AAAA, CNAME, or TXT)"
            )),
        }
    }
}

/// Validate user-supplied values for `record_type` and convert them to the
/// wire form Route 53 expects. Runs before any network call, so a malformed
/// address never reaches AWS.
pub fn encode_record_values(record_type: RecordType, values: &[String]) -> Result<Vec<String>> {
    if values.is_empty() {
        return Err(Error::InvalidRequest(format!(
            "at least one {} record value is required",
            record_type.as_str()
        )));
    }
    let invalid = |value: &str, expected: &str| {
        Error::InvalidRequest(format!(
            "`{value}` is not a valid {expected} for a {} record",
            record_type.as_str()
        ))
    };
    match record_type {
        RecordType::A => values
            .iter()
            .map(|value| {
                value
                    .parse::<std::net::Ipv4Addr>()
                    .map(|address| address.to_string())
                    .map_err(|_| invalid(value, "IPv4 address"))
            })
            .collect(),
        RecordType::Aaaa => values
            .iter()
            .map(|value| {
                value
                    .parse::<std::net::Ipv6Addr>()
                    .map(|address| address.to_string())
                    .map_err(|_| invalid(value, "IPv6 address"))
            })
            .collect(),
        RecordType::Cname => {
            if values.len() != 1 {
                return Err(Error::InvalidRequest(
                    "a CNAME record takes exactly one --value".into(),
                ));
            }
            let target = values[0].trim_end_matches('.');
            let valid = !target.is_empty()
                && target.split('.').all(|label| {
                    !label.is_empty()
                        && label.len() <= 63
                        && label
                            .bytes()
                            .all(|byte| byte.is_ascii_alphanumeric() || b"-_".contains(&byte))
                });
            if !valid {
                return Err(invalid(&values[0], "hostname"));
            }
            Ok(vec![format!("{target}.")])
        }
        RecordType::Txt => Ok(values.iter().map(|value| quote_txt_value(value)).collect()),
    }
}

/// Find the hosted zone that is authoritative for `name`: the public zone with
/// the longest name that equals `name` or is a parent domain of it.
pub fn find_zone_id(client: &AwsClient, name: &str) -> Result<String> {
    select_zone(&list_zone_summaries(client)?, name)
}

fn select_zone(zones: &[HostedZoneXml], name: &str) -> Result<String> {
    let name = normalize_name(name);
    let mut best: Vec<&HostedZoneXml> = Vec::new();
    let mut best_len = 0;
    for zone in zones {
        if zone.config.private_zone.unwrap_or(false) {
            continue;
        }
        let zone_name = normalize_name(&zone.name);
        let matches = name == zone_name || name.ends_with(&format!(".{zone_name}"));
        if !matches || zone_name.len() < best_len {
            continue;
        }
        if zone_name.len() > best_len {
            best.clear();
            best_len = zone_name.len();
        }
        best.push(zone);
    }
    match best.as_slice() {
        [zone] => Ok(zone.id.rsplit('/').next().unwrap_or(&zone.id).to_owned()),
        [] => Err(Error::InvalidRequest(format!(
            "no public hosted zone in this account contains `{name}`"
        ))),
        _ => Err(Error::InvalidRequest(format!(
            "several public hosted zones named `{}` exist; pass --zone-id",
            normalize_name(&best[0].name)
        ))),
    }
}

fn normalize_name(name: &str) -> String {
    name.trim_end_matches('.').to_ascii_lowercase()
}

#[allow(clippy::too_many_arguments)]
pub fn change_record(
    client: &AwsClient,
    hosted_zone_id: &str,
    action: RecordChangeAction,
    record_type: RecordType,
    name: &str,
    ttl: u64,
    values: &[String],
    comment: &str,
) -> Result<ChangeResponse> {
    let hosted_zone_id = hosted_zone_id
        .strip_prefix("/hostedzone/")
        .unwrap_or(hosted_zone_id);
    if hosted_zone_id.is_empty() {
        return Err(Error::InvalidRequest(
            "hosted zone ID cannot be empty".into(),
        ));
    }
    if name.is_empty() {
        return Err(Error::InvalidRequest("record name cannot be empty".into()));
    }
    if ttl == 0 {
        return Err(Error::InvalidRequest("record TTL must be positive".into()));
    }

    let resource_records = ChangeResourceRecords {
        values: encode_record_values(record_type, values)?
            .into_iter()
            .map(|value| ChangeResourceRecord { value })
            .collect(),
    };
    rest_xml::post_xml(
        client,
        "route53",
        "us-east-1",
        ENDPOINT,
        &format!("{PREFIX}/hostedzone/{hosted_zone_id}/rrset"),
        &ChangeRecordsRequest {
            xmlns: NAMESPACE,
            change_batch: ChangeBatch {
                comment,
                changes: Changes {
                    values: vec![RecordChange {
                        action: action.as_str(),
                        resource_record_set: RecordSetChange {
                            name,
                            r#type: record_type.as_str(),
                            ttl,
                            resource_records,
                        },
                    }],
                },
            },
        },
    )
}

/// Route 53 stores a TXT value as one or more quoted character strings, each
/// at most 255 bytes long. Values longer than that (DKIM keys, long SPF) must
/// be split into consecutive quoted chunks separated by a space; resolvers
/// concatenate them back.
fn quote_txt_value(value: &str) -> String {
    const CHUNK: usize = 255;
    let escaped = value.replace('\\', "\\\\").replace('"', "\\\"");
    if escaped.len() <= CHUNK {
        return format!("\"{escaped}\"");
    }
    let mut chunks = Vec::new();
    let mut start = 0;
    let bytes = escaped.as_bytes();
    while start < bytes.len() {
        let mut end = (start + CHUNK).min(bytes.len());
        // Never split a UTF-8 sequence or separate a backslash from what it escapes.
        while end < bytes.len() && !escaped.is_char_boundary(end) {
            end -= 1;
        }
        if end < bytes.len() && trailing_backslashes(&bytes[start..end]) % 2 == 1 {
            end -= 1;
        }
        chunks.push(format!("\"{}\"", &escaped[start..end]));
        start = end;
    }
    chunks.join(" ")
}

fn trailing_backslashes(bytes: &[u8]) -> usize {
    bytes
        .iter()
        .rev()
        .take_while(|byte| **byte == b'\\')
        .count()
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

    #[test]
    fn decodes_ttl_acronym() {
        let response: RecordsResponse = quick_xml::de::from_str(
            r#"<ListResourceRecordSetsResponse><ResourceRecordSets><ResourceRecordSet><Name>example.com.</Name><Type>A</Type><TTL>300</TTL><ResourceRecords><ResourceRecord><Value>192.0.2.1</Value></ResourceRecord></ResourceRecords></ResourceRecordSet></ResourceRecordSets></ListResourceRecordSetsResponse>"#,
        )
        .unwrap();
        assert_eq!(response.resource_record_sets.values[0].ttl, Some(300));
    }

    #[test]
    fn change_txt_payload_has_route53_wire_shape_and_escaping() {
        let request = ChangeRecordsRequest {
            xmlns: NAMESPACE,
            change_batch: ChangeBatch {
                comment: "Let's Encrypt & friends",
                changes: Changes {
                    values: vec![RecordChange {
                        action: RecordChangeAction::Upsert.as_str(),
                        resource_record_set: RecordSetChange {
                            name: "_acme-challenge.example.com",
                            r#type: "TXT",
                            ttl: 60,
                            resource_records: ChangeResourceRecords {
                                values: vec![ChangeResourceRecord {
                                    value: quote_txt_value("proof-token"),
                                }],
                            },
                        },
                    }],
                },
            },
        };
        let xml = quick_xml::se::to_string(&request).unwrap();
        assert!(xml.contains("<Action>UPSERT</Action>"));
        assert!(xml.contains("<Name>_acme-challenge.example.com</Name>"));
        assert!(xml.contains("<Type>TXT</Type><TTL>60</TTL>"));
        assert!(xml.contains("<Value>\"proof-token\"</Value>"), "{xml}");
        assert!(xml.contains("Let's Encrypt &amp; friends"));
        assert!(xml.contains("xmlns=\"https://route53.amazonaws.com/doc/2013-04-01/\""));
    }

    #[test]
    fn txt_values_are_quoted_for_route53() {
        assert_eq!(quote_txt_value("proof-token"), r#""proof-token""#);
        assert_eq!(quote_txt_value(r#"a\b"c"#), r#""a\\b\"c""#);
    }

    #[test]
    fn long_txt_values_are_split_into_255_byte_strings() {
        let long = "a".repeat(600);
        let quoted = quote_txt_value(&long);
        let parts: Vec<&str> = quoted.split(' ').collect();
        assert_eq!(parts.len(), 3);
        assert_eq!(parts[0].len(), 257);
        assert_eq!(parts[2], format!("\"{}\"", "a".repeat(90)));
        assert_eq!(quoted.replace(['"', ' '], ""), long);
    }

    #[test]
    fn txt_split_never_strands_an_escape_or_splits_a_char() {
        let mut value = "x".repeat(254);
        value.push('"');
        value.push_str(&"y".repeat(10));
        let quoted = quote_txt_value(&value);
        for part in quoted.split(' ') {
            let inner = &part[1..part.len() - 1];
            assert!(inner.len() <= 255);
            assert_eq!(trailing_backslashes(inner.as_bytes()) % 2, 0, "{part}");
        }
        let multibyte = "é".repeat(200);
        for part in quote_txt_value(&multibyte).split(' ') {
            assert!(part.len() - 2 <= 255);
        }
    }

    fn zone(id: &str, name: &str, private: bool) -> HostedZoneXml {
        HostedZoneXml {
            id: format!("/hostedzone/{id}"),
            name: name.into(),
            config: ZoneConfig {
                private_zone: Some(private),
                comment: None,
            },
        }
    }

    #[test]
    fn record_type_parses_case_insensitively() {
        assert_eq!("a".parse::<RecordType>(), Ok(RecordType::A));
        assert_eq!("AAAA".parse::<RecordType>(), Ok(RecordType::Aaaa));
        assert_eq!("Cname".parse::<RecordType>(), Ok(RecordType::Cname));
        assert_eq!("txt".parse::<RecordType>(), Ok(RecordType::Txt));
        assert!("MX".parse::<RecordType>().is_err());
    }

    #[test]
    fn a_and_aaaa_values_must_be_addresses_of_the_right_family() {
        let ok = encode_record_values(RecordType::A, &["170.75.167.194".into()]).unwrap();
        assert_eq!(ok, vec!["170.75.167.194"]);
        assert!(encode_record_values(RecordType::A, &["2001:db8::1".into()]).is_err());
        assert!(encode_record_values(RecordType::A, &["example.com".into()]).is_err());
        let v6 = encode_record_values(RecordType::Aaaa, &["2001:DB8:0::1".into()]).unwrap();
        assert_eq!(v6, vec!["2001:db8::1"]);
        assert!(encode_record_values(RecordType::Aaaa, &["192.0.2.1".into()]).is_err());
        assert!(encode_record_values(RecordType::A, &[]).is_err());
    }

    #[test]
    fn cname_takes_one_hostname_and_is_made_absolute() {
        let target = encode_record_values(RecordType::Cname, &["host.example.com".into()]).unwrap();
        assert_eq!(target, vec!["host.example.com."]);
        let absolute =
            encode_record_values(RecordType::Cname, &["host.example.com.".into()]).unwrap();
        assert_eq!(absolute, vec!["host.example.com."]);
        assert!(
            encode_record_values(RecordType::Cname, &["a.com".into(), "b.com".into()]).is_err()
        );
        assert!(encode_record_values(RecordType::Cname, &["bad host".into()]).is_err());
        assert!(encode_record_values(RecordType::Cname, &["a..com".into()]).is_err());
    }

    #[test]
    fn zone_selection_prefers_the_longest_public_parent() {
        let zones = vec![
            zone("ZAPEX", "xmemory.ai.", false),
            zone("ZSTG", "stg.xmemory.ai.", false),
            zone("ZPRIV", "stg.xmemory.ai.", true),
            zone("ZDIMA", "dima.ai.", false),
        ];
        assert_eq!(select_zone(&zones, "api.stg.xmemory.ai").unwrap(), "ZSTG");
        assert_eq!(select_zone(&zones, "www.xmemory.ai.").unwrap(), "ZAPEX");
        assert_eq!(select_zone(&zones, "CTL.Dima.AI").unwrap(), "ZDIMA");
        assert_eq!(select_zone(&zones, "dima.ai").unwrap(), "ZDIMA");
        assert!(select_zone(&zones, "notdima.ai").is_err());
        assert!(select_zone(&zones, "example.com").is_err());
    }

    #[test]
    fn zone_selection_refuses_to_guess_between_duplicate_zones() {
        let zones = vec![zone("Z1", "dima.ai.", false), zone("Z2", "dima.ai.", false)];
        let error = select_zone(&zones, "ctl.dima.ai").unwrap_err().to_string();
        assert!(error.contains("--zone-id"), "{error}");
    }

    #[test]
    fn change_a_payload_has_route53_wire_shape() {
        let request = ChangeRecordsRequest {
            xmlns: NAMESPACE,
            change_batch: ChangeBatch {
                comment: "",
                changes: Changes {
                    values: vec![RecordChange {
                        action: RecordChangeAction::Upsert.as_str(),
                        resource_record_set: RecordSetChange {
                            name: "ctl.example.com",
                            r#type: RecordType::A.as_str(),
                            ttl: 300,
                            resource_records: ChangeResourceRecords {
                                values: vec![ChangeResourceRecord {
                                    value: "192.0.2.10".into(),
                                }],
                            },
                        },
                    }],
                },
            },
        };
        let xml = quick_xml::se::to_string(&request).unwrap();
        assert!(xml.contains("<Type>A</Type><TTL>300</TTL>"), "{xml}");
        assert!(xml.contains("<Value>192.0.2.10</Value>"), "{xml}");
        assert!(!xml.contains("<Comment>"), "{xml}");
    }

    #[test]
    fn decodes_change_response() {
        let response: ChangeResponse = quick_xml::de::from_str(
            r#"<ChangeResourceRecordSetsResponse><ChangeInfo><Id>/change/C123</Id><Status>PENDING</Status><SubmittedAt>2026-07-31T12:00:00Z</SubmittedAt></ChangeInfo></ChangeResourceRecordSetsResponse>"#,
        )
        .unwrap();
        assert_eq!(response.change_info.id, "/change/C123");
        assert_eq!(response.change_info.status, "PENDING");
    }
}
