use serde::{Deserialize, Serialize, de::DeserializeOwned};

use crate::{AwsClient, Result, protocols::query};

use super::common::Items;

const VERSION: &str = "2016-11-15";

fn call<T: DeserializeOwned>(
    client: &AwsClient,
    region: &str,
    operation: &str,
    parameters: &[(String, String)],
) -> Result<T> {
    query::call(
        client,
        &format!("ec2.{region}.amazonaws.com"),
        "ec2",
        region,
        operation,
        VERSION,
        parameters,
    )
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RegionsResponse {
    #[serde(default)]
    region_info: Items<Region>,
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
struct ReservationsPage {
    #[serde(default)]
    reservation_set: Items<Reservation>,
    next_token: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Reservation {
    #[serde(default)]
    instances_set: Items<InstanceXml>,
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
    tag_set: Items<Tag>,
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

#[derive(Debug, Deserialize)]
struct Tag {
    key: String,
    value: String,
}

fn name_tag(tags: Items<Tag>) -> Option<String> {
    tags.items
        .into_iter()
        .find(|tag| tag.key == "Name")
        .map(|tag| tag.value)
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

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SecurityGroupsPage {
    #[serde(default)]
    security_group_info: Items<SecurityGroupXml>,
    next_token: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SecurityGroupXml {
    group_id: String,
    group_name: Option<String>,
    vpc_id: Option<String>,
    group_description: Option<String>,
    #[serde(default)]
    ip_permissions: Items<Permission>,
    #[serde(default)]
    ip_permissions_egress: Items<Permission>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Permission {
    ip_protocol: Option<String>,
    from_port: Option<i32>,
    to_port: Option<i32>,
    #[serde(default)]
    ip_ranges: Items<Ipv4Range>,
    #[serde(default)]
    ipv6_ranges: Items<Ipv6Range>,
    #[serde(default)]
    groups: Items<GroupReference>,
    #[serde(default)]
    prefix_list_ids: Items<PrefixList>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Ipv4Range {
    cidr_ip: Option<String>,
}
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Ipv6Range {
    cidr_ipv6: Option<String>,
}
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GroupReference {
    group_id: Option<String>,
}
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PrefixList {
    prefix_list_id: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct SecurityRule {
    pub protocol: Option<String>,
    pub from_port: Option<i32>,
    pub to_port: Option<i32>,
    pub ipv4_ranges: Vec<String>,
    pub ipv6_ranges: Vec<String>,
    pub referenced_groups: Vec<String>,
    pub prefix_lists: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct SecurityGroup {
    pub region: String,
    pub group_id: String,
    pub name: Option<String>,
    pub vpc_id: Option<String>,
    pub ingress_rules: usize,
    pub egress_rules: usize,
    pub ingress: Vec<SecurityRule>,
    pub egress: Vec<SecurityRule>,
    pub description: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct VolumesPage {
    #[serde(default)]
    volume_set: Items<VolumeXml>,
    next_token: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct VolumeXml {
    volume_id: String,
    availability_zone: Option<String>,
    size: Option<u64>,
    volume_type: Option<String>,
    status: Option<String>,
    #[serde(default)]
    attachment_set: Items<Attachment>,
    #[serde(default)]
    tag_set: Items<Tag>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Attachment {
    instance_id: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct Volume {
    pub region: String,
    pub availability_zone: Option<String>,
    pub volume_id: String,
    pub name: Option<String>,
    pub size_gib: Option<u64>,
    pub volume_type: Option<String>,
    pub state: Option<String>,
    pub attached_to: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AddressesPage {
    #[serde(default)]
    addresses_set: Items<AddressXml>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AddressXml {
    public_ip: Option<String>,
    instance_id: Option<String>,
    network_interface_id: Option<String>,
    association_id: Option<String>,
    #[serde(default)]
    tag_set: Items<Tag>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct Address {
    pub region: String,
    pub public_ip: Option<String>,
    pub name: Option<String>,
    pub instance_id: Option<String>,
    pub network_interface_id: Option<String>,
    pub associated: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SnapshotsPage {
    #[serde(default)]
    snapshot_set: Items<SnapshotXml>,
    next_token: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SnapshotXml {
    snapshot_id: String,
    volume_size: Option<u64>,
    status: Option<String>,
    start_time: Option<String>,
    description: Option<String>,
    #[serde(default)]
    tag_set: Items<Tag>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct Snapshot {
    pub region: String,
    pub snapshot_id: String,
    pub name: Option<String>,
    pub size_gib: Option<u64>,
    pub state: Option<String>,
    pub started: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ImagesPage {
    #[serde(default)]
    images_set: Items<ImageXml>,
    next_token: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ImageXml {
    image_id: String,
    name: Option<String>,
    creation_date: Option<String>,
    image_state: Option<String>,
    is_public: Option<bool>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct Image {
    pub region: String,
    pub image_id: String,
    pub name: Option<String>,
    pub created: Option<String>,
    pub state: Option<String>,
    pub public: Option<bool>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct VpcEndpointsPage {
    #[serde(default)]
    vpc_endpoint_set: Items<VpcEndpointXml>,
    next_token: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct VpcEndpointXml {
    vpc_endpoint_id: String,
    vpc_endpoint_type: Option<String>,
    service_name: Option<String>,
    vpc_id: Option<String>,
    state: Option<String>,
    #[serde(default)]
    tag_set: Items<Tag>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct VpcEndpoint {
    pub region: String,
    pub vpc_endpoint_id: String,
    pub name: Option<String>,
    pub endpoint_type: Option<String>,
    pub service: Option<String>,
    pub vpc_id: Option<String>,
    pub state: Option<String>,
}

pub fn describe_regions(client: &AwsClient, region: &str) -> Result<Vec<Region>> {
    let response: RegionsResponse = call(client, region, "DescribeRegions", &[])?;
    Ok(response.region_info.items)
}

macro_rules! typed_pages {
    ($client:expr, $region:expr, $operation:expr, $base:expr, $page:ty, $field:ident) => {{
        let mut output = Vec::new();
        let mut parameters = $base.to_vec();
        loop {
            let response: $page = call($client, $region, $operation, &parameters)?;
            output.extend(response.$field.items);
            let Some(token) = response.next_token else {
                break;
            };
            parameters.retain(|(key, _)| key != "NextToken");
            parameters.push(("NextToken".into(), token));
        }
        output
    }};
}

pub fn describe_instances(client: &AwsClient, region: &str) -> Result<Vec<Instance>> {
    let reservations = typed_pages!(
        client,
        region,
        "DescribeInstances",
        &[],
        ReservationsPage,
        reservation_set
    );
    Ok(reservations
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
                    name: name_tag(instance.tag_set),
                    instance_type: instance.instance_type,
                    state: instance.instance_state.and_then(|state| state.name),
                    public_ip: instance.public_ip_address,
                    private_ip: instance.private_ip_address,
                })
        })
        .collect())
}

pub fn describe_security_groups(client: &AwsClient, region: &str) -> Result<Vec<SecurityGroup>> {
    let groups = typed_pages!(
        client,
        region,
        "DescribeSecurityGroups",
        &[],
        SecurityGroupsPage,
        security_group_info
    );
    Ok(groups
        .into_iter()
        .map(|group| SecurityGroup {
            region: region.into(),
            group_id: group.group_id,
            name: group.group_name,
            vpc_id: group.vpc_id,
            ingress_rules: group.ip_permissions.items.len(),
            egress_rules: group.ip_permissions_egress.items.len(),
            ingress: group.ip_permissions.items.into_iter().map(rule).collect(),
            egress: group
                .ip_permissions_egress
                .items
                .into_iter()
                .map(rule)
                .collect(),
            description: group.group_description,
        })
        .collect())
}

fn rule(value: Permission) -> SecurityRule {
    SecurityRule {
        protocol: value.ip_protocol,
        from_port: value.from_port,
        to_port: value.to_port,
        ipv4_ranges: value
            .ip_ranges
            .items
            .into_iter()
            .filter_map(|x| x.cidr_ip)
            .collect(),
        ipv6_ranges: value
            .ipv6_ranges
            .items
            .into_iter()
            .filter_map(|x| x.cidr_ipv6)
            .collect(),
        referenced_groups: value
            .groups
            .items
            .into_iter()
            .filter_map(|x| x.group_id)
            .collect(),
        prefix_lists: value
            .prefix_list_ids
            .items
            .into_iter()
            .filter_map(|x| x.prefix_list_id)
            .collect(),
    }
}

pub fn describe_volumes(client: &AwsClient, region: &str) -> Result<Vec<Volume>> {
    let volumes = typed_pages!(
        client,
        region,
        "DescribeVolumes",
        &[],
        VolumesPage,
        volume_set
    );
    Ok(volumes
        .into_iter()
        .map(|volume| Volume {
            region: region.into(),
            availability_zone: volume.availability_zone,
            volume_id: volume.volume_id,
            name: name_tag(volume.tag_set),
            size_gib: volume.size,
            volume_type: volume.volume_type,
            state: volume.status,
            attached_to: volume
                .attachment_set
                .items
                .into_iter()
                .filter_map(|a| a.instance_id)
                .collect(),
        })
        .collect())
}

pub fn describe_addresses(client: &AwsClient, region: &str) -> Result<Vec<Address>> {
    let page: AddressesPage = call(client, region, "DescribeAddresses", &[])?;
    Ok(page
        .addresses_set
        .items
        .into_iter()
        .map(|address| Address {
            region: region.into(),
            public_ip: address.public_ip,
            name: name_tag(address.tag_set),
            instance_id: address.instance_id,
            network_interface_id: address.network_interface_id,
            associated: address.association_id.is_some(),
        })
        .collect())
}

pub fn describe_snapshots(client: &AwsClient, region: &str) -> Result<Vec<Snapshot>> {
    let base = vec![("Owner.1".into(), "self".into())];
    let snapshots = typed_pages!(
        client,
        region,
        "DescribeSnapshots",
        &base,
        SnapshotsPage,
        snapshot_set
    );
    Ok(snapshots
        .into_iter()
        .map(|snapshot| Snapshot {
            region: region.into(),
            snapshot_id: snapshot.snapshot_id,
            name: name_tag(snapshot.tag_set),
            size_gib: snapshot.volume_size,
            state: snapshot.status,
            started: snapshot.start_time,
            description: snapshot.description,
        })
        .collect())
}

pub fn describe_images(client: &AwsClient, region: &str) -> Result<Vec<Image>> {
    let base = vec![("Owner.1".into(), "self".into())];
    let images = typed_pages!(
        client,
        region,
        "DescribeImages",
        &base,
        ImagesPage,
        images_set
    );
    Ok(images
        .into_iter()
        .map(|image| Image {
            region: region.into(),
            image_id: image.image_id,
            name: image.name,
            created: image.creation_date,
            state: image.image_state,
            public: image.is_public,
        })
        .collect())
}

pub fn describe_vpc_endpoints(client: &AwsClient, region: &str) -> Result<Vec<VpcEndpoint>> {
    let endpoints = typed_pages!(
        client,
        region,
        "DescribeVpcEndpoints",
        &[],
        VpcEndpointsPage,
        vpc_endpoint_set
    );
    Ok(endpoints
        .into_iter()
        .map(|endpoint| VpcEndpoint {
            region: region.into(),
            vpc_endpoint_id: endpoint.vpc_endpoint_id,
            name: name_tag(endpoint.tag_set),
            endpoint_type: endpoint.vpc_endpoint_type,
            service: endpoint.service_name,
            vpc_id: endpoint.vpc_id,
            state: endpoint.state,
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
