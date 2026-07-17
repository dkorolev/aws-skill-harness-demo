use serde::{Deserialize, Serialize};

use crate::{AwsClient, Result, protocols::query};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RegionsResponse {
    #[serde(default)]
    region_info: RegionItems,
}

#[derive(Debug, Default, Deserialize)]
struct RegionItems {
    #[serde(rename = "item", default)]
    items: Vec<Region>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct Region {
    #[serde(rename(deserialize = "regionName"))]
    pub region_name: String,
    #[serde(rename(deserialize = "regionEndpoint"))]
    pub endpoint: Option<String>,
    #[serde(rename(deserialize = "optInStatus"))]
    pub opt_in_status: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct InstancesResponse {
    #[serde(default)]
    reservation_set: ReservationItems,
}

#[derive(Debug, Default, Deserialize)]
struct ReservationItems {
    #[serde(rename = "item", default)]
    items: Vec<Reservation>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Reservation {
    #[serde(default)]
    instances_set: InstanceItems,
}

#[derive(Debug, Default, Deserialize)]
struct InstanceItems {
    #[serde(rename = "item", default)]
    items: Vec<InstanceXml>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct InstanceXml {
    instance_id: String,
    instance_type: Option<String>,
    public_ip_address: Option<String>,
    private_ip_address: Option<String>,
    placement: Option<Placement>,
    instance_state: Option<InstanceState>,
    #[serde(default)]
    tag_set: TagItems,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Placement {
    availability_zone: Option<String>,
}

#[derive(Debug, Deserialize)]
struct InstanceState {
    name: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
struct TagItems {
    #[serde(rename = "item", default)]
    items: Vec<Tag>,
}

#[derive(Debug, Deserialize)]
struct Tag {
    key: String,
    value: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct Instance {
    pub region: String,
    pub availability_zone: Option<String>,
    pub instance_id: String,
    pub name: Option<String>,
    pub instance_type: Option<String>,
    pub state: Option<String>,
    pub public_ip: Option<String>,
    pub private_ip: Option<String>,
}

pub fn describe_regions(client: &AwsClient, region: &str) -> Result<Vec<Region>> {
    let response: RegionsResponse = query::call(
        client,
        &format!("ec2.{region}.amazonaws.com"),
        "ec2",
        region,
        "DescribeRegions",
        "2016-11-15",
        &[],
    )?;
    Ok(response.region_info.items)
}

pub fn describe_instances(client: &AwsClient, region: &str) -> Result<Vec<Instance>> {
    let response: InstancesResponse = query::call(
        client,
        &format!("ec2.{region}.amazonaws.com"),
        "ec2",
        region,
        "DescribeInstances",
        "2016-11-15",
        &[],
    )?;
    Ok(response
        .reservation_set
        .items
        .into_iter()
        .flat_map(|reservation| {
            reservation
                .instances_set
                .items
                .into_iter()
                .map(|instance| Instance {
                    region: region.to_owned(),
                    availability_zone: instance
                        .placement
                        .and_then(|placement| placement.availability_zone),
                    instance_id: instance.instance_id,
                    name: instance
                        .tag_set
                        .items
                        .into_iter()
                        .find(|tag| tag.key == "Name")
                        .map(|tag| tag.value),
                    instance_type: instance.instance_type,
                    state: instance.instance_state.and_then(|state| state.name),
                    public_ip: instance.public_ip_address,
                    private_ip: instance.private_ip_address,
                })
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_ec2_query_response_without_line_format_assumptions() {
        let xml = br#"<DescribeRegionsResponse xmlns="http://ec2.amazonaws.com/doc/2016-11-15/"><requestId>x</requestId><regionInfo><item><regionName>eu-west-2</regionName><regionEndpoint>ec2.eu-west-2.amazonaws.com</regionEndpoint><optInStatus>opt-in-not-required</optInStatus></item></regionInfo></DescribeRegionsResponse>"#;
        let response: RegionsResponse = quick_xml::de::from_reader(xml.as_slice()).unwrap();
        assert_eq!(response.region_info.items[0].region_name, "eu-west-2");
    }
}
