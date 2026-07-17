use serde::{Deserialize, Serialize};

use crate::{AwsClient, Result, protocols::query};

use super::common::Members;

const VERSION: &str = "2015-12-01";

fn call<T: for<'de> Deserialize<'de>>(
    client: &AwsClient,
    region: &str,
    operation: &str,
    parameters: &[(String, String)],
) -> Result<T> {
    query::call(
        client,
        &format!("elasticloadbalancing.{region}.amazonaws.com"),
        "elasticloadbalancing",
        region,
        operation,
        VERSION,
        parameters,
    )
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct LoadBalancersResponse {
    describe_load_balancers_result: LoadBalancersResult,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct LoadBalancersResult {
    #[serde(default)]
    load_balancers: Members<LoadBalancerXml>,
    next_marker: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct LoadBalancerXml {
    load_balancer_arn: String,
    load_balancer_name: String,
    #[serde(rename = "DNSName")]
    dns_name: Option<String>,
    scheme: Option<String>,
    r#type: Option<String>,
    state: Option<State>,
    #[serde(default)]
    availability_zones: Members<AvailabilityZone>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct State {
    code: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct AvailabilityZone {
    zone_name: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct LoadBalancer {
    pub region: String,
    pub arn: String,
    pub name: String,
    pub load_balancer_type: Option<String>,
    pub scheme: Option<String>,
    pub state: Option<String>,
    pub availability_zones: Vec<String>,
    pub dns_name: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct TargetGroupsResponse {
    describe_target_groups_result: TargetGroupsResult,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct TargetGroupsResult {
    #[serde(default)]
    target_groups: Members<TargetGroupXml>,
    next_marker: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct TargetGroupXml {
    target_group_arn: String,
    target_group_name: String,
    protocol: Option<String>,
    port: Option<u16>,
    target_type: Option<String>,
    #[serde(default)]
    load_balancer_arns: Members<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct TargetHealthResponse {
    describe_target_health_result: TargetHealthResult,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct TargetHealthResult {
    #[serde(default)]
    target_health_descriptions: Members<TargetHealthDescription>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct TargetHealthDescription {
    target_health: Option<TargetHealth>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct TargetHealth {
    state: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct TargetGroup {
    pub region: String,
    pub arn: String,
    pub name: String,
    pub protocol: Option<String>,
    pub port: Option<u16>,
    pub target_type: Option<String>,
    pub healthy_targets: usize,
    pub total_targets: usize,
    pub load_balancers: usize,
}

pub fn describe_load_balancers(client: &AwsClient, region: &str) -> Result<Vec<LoadBalancer>> {
    let mut marker: Option<String> = None;
    let mut output = Vec::new();
    loop {
        let parameters = marker
            .iter()
            .map(|value| ("Marker".into(), value.clone()))
            .collect::<Vec<_>>();
        let response: LoadBalancersResponse =
            call(client, region, "DescribeLoadBalancers", &parameters)?;
        output.extend(
            response
                .describe_load_balancers_result
                .load_balancers
                .members
                .into_iter()
                .map(|lb| LoadBalancer {
                    region: region.into(),
                    arn: lb.load_balancer_arn,
                    name: lb.load_balancer_name,
                    load_balancer_type: lb.r#type,
                    scheme: lb.scheme,
                    state: lb.state.and_then(|state| state.code),
                    availability_zones: lb
                        .availability_zones
                        .members
                        .into_iter()
                        .filter_map(|zone| zone.zone_name)
                        .collect(),
                    dns_name: lb.dns_name,
                }),
        );
        marker = response.describe_load_balancers_result.next_marker;
        if marker.is_none() {
            break;
        }
    }
    Ok(output)
}

pub fn describe_target_groups(client: &AwsClient, region: &str) -> Result<Vec<TargetGroup>> {
    let mut marker: Option<String> = None;
    let mut groups = Vec::new();
    loop {
        let parameters = marker
            .iter()
            .map(|value| ("Marker".into(), value.clone()))
            .collect::<Vec<_>>();
        let response: TargetGroupsResponse =
            call(client, region, "DescribeTargetGroups", &parameters)?;
        groups.extend(response.describe_target_groups_result.target_groups.members);
        marker = response.describe_target_groups_result.next_marker;
        if marker.is_none() {
            break;
        }
    }
    let mut output = Vec::new();
    for group in groups {
        let health: TargetHealthResponse = call(
            client,
            region,
            "DescribeTargetHealth",
            &[("TargetGroupArn".into(), group.target_group_arn.clone())],
        )?;
        let targets = health
            .describe_target_health_result
            .target_health_descriptions
            .members;
        let healthy = targets
            .iter()
            .filter(|target| {
                target
                    .target_health
                    .as_ref()
                    .and_then(|health| health.state.as_deref())
                    == Some("healthy")
            })
            .count();
        output.push(TargetGroup {
            region: region.into(),
            arn: group.target_group_arn,
            name: group.target_group_name,
            protocol: group.protocol,
            port: group.port,
            target_type: group.target_type,
            healthy_targets: healthy,
            total_targets: targets.len(),
            load_balancers: group.load_balancer_arns.members.len(),
        });
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_aws_dns_acronym() {
        let response: LoadBalancersResponse = quick_xml::de::from_str(
            r#"<DescribeLoadBalancersResponse><DescribeLoadBalancersResult><LoadBalancers><member><LoadBalancerArn>arn:lb</LoadBalancerArn><LoadBalancerName>demo</LoadBalancerName><DNSName>demo.elb.amazonaws.com</DNSName></member></LoadBalancers></DescribeLoadBalancersResult></DescribeLoadBalancersResponse>"#,
        )
        .unwrap();
        assert_eq!(
            response
                .describe_load_balancers_result
                .load_balancers
                .members[0]
                .dns_name
                .as_deref(),
            Some("demo.elb.amazonaws.com")
        );
    }
}
