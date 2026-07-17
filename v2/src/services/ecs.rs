use serde::{Deserialize, Serialize};

use crate::{AwsClient, Result, protocols::json};

const PREFIX: &str = "AmazonEC2ContainerServiceV20141113";

#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
struct ListRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    cluster: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    next_token: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ListClustersResponse {
    #[serde(default)]
    cluster_arns: Vec<String>,
    next_token: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ListServicesResponse {
    #[serde(default)]
    service_arns: Vec<String>,
    next_token: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct DescribeRequest {
    cluster: String,
    services: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct DescribeResponse {
    #[serde(default)]
    services: Vec<ServiceXml>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ServiceXml {
    service_name: String,
    status: Option<String>,
    running_count: Option<u64>,
    desired_count: Option<u64>,
    launch_type: Option<String>,
    #[serde(default)]
    capacity_provider_strategy: Vec<CapacityProvider>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CapacityProvider {
    capacity_provider: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct Service {
    pub region: String,
    pub cluster: String,
    pub service: Option<String>,
    pub status: Option<String>,
    pub running: Option<u64>,
    pub desired: Option<u64>,
    pub launch: Vec<String>,
}

pub fn list_services(client: &AwsClient, region: &str) -> Result<Vec<Service>> {
    let endpoint = format!("ecs.{region}.amazonaws.com");
    let mut clusters = Vec::new();
    let mut token = None;
    loop {
        let response: ListClustersResponse = json::call(
            client,
            "ecs",
            region,
            &endpoint,
            &format!("{PREFIX}.ListClusters"),
            &ListRequest {
                cluster: None,
                next_token: token,
            },
        )?;
        clusters.extend(response.cluster_arns);
        token = response.next_token;
        if token.is_none() {
            break;
        }
    }
    let mut output = Vec::new();
    for arn in clusters {
        let cluster = arn.rsplit('/').next().unwrap_or(&arn).to_owned();
        let mut service_arns = Vec::new();
        token = None;
        loop {
            let response: ListServicesResponse = json::call(
                client,
                "ecs",
                region,
                &endpoint,
                &format!("{PREFIX}.ListServices"),
                &ListRequest {
                    cluster: Some(cluster.clone()),
                    next_token: token,
                },
            )?;
            service_arns.extend(response.service_arns);
            token = response.next_token;
            if token.is_none() {
                break;
            }
        }
        if service_arns.is_empty() {
            output.push(Service {
                region: region.into(),
                cluster,
                service: None,
                status: None,
                running: None,
                desired: None,
                launch: Vec::new(),
            });
            continue;
        }
        for chunk in service_arns.chunks(10) {
            let response: DescribeResponse = json::call(
                client,
                "ecs",
                region,
                &endpoint,
                &format!("{PREFIX}.DescribeServices"),
                &DescribeRequest {
                    cluster: cluster.clone(),
                    services: chunk.to_vec(),
                },
            )?;
            output.extend(response.services.into_iter().map(|service| {
                Service {
                    region: region.into(),
                    cluster: cluster.clone(),
                    service: Some(service.service_name),
                    status: service.status,
                    running: service.running_count,
                    desired: service.desired_count,
                    launch: service
                        .launch_type
                        .into_iter()
                        .chain(
                            service
                                .capacity_provider_strategy
                                .into_iter()
                                .map(|provider| provider.capacity_provider),
                        )
                        .collect(),
                }
            }));
        }
    }
    Ok(output)
}
