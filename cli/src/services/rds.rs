use serde::{Deserialize, Serialize};

use crate::{AwsClient, Result, protocols::query};

const VERSION: &str = "2014-10-31";

fn call<T: for<'de> Deserialize<'de>>(
    client: &AwsClient,
    region: &str,
    operation: &str,
    parameters: &[(String, String)],
) -> Result<T> {
    query::call(
        client,
        &format!("rds.{region}.amazonaws.com"),
        "rds",
        region,
        operation,
        VERSION,
        parameters,
    )
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct InstancesResponse {
    #[serde(rename = "DescribeDBInstancesResult")]
    describe_db_instances_result: InstancesResult,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct InstancesResult {
    #[serde(rename = "DBInstances", default)]
    db_instances: DbInstances,
    marker: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
struct DbInstances {
    #[serde(rename = "DBInstance", default)]
    values: Vec<InstanceXml>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct InstanceXml {
    #[serde(rename = "DBInstanceIdentifier")]
    db_instance_identifier: String,
    #[serde(rename = "DBInstanceClass")]
    db_instance_class: Option<String>,
    engine: Option<String>,
    engine_version: Option<String>,
    #[serde(rename = "DBInstanceStatus")]
    db_instance_status: Option<String>,
    availability_zone: Option<String>,
    endpoint: Option<Endpoint>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct Endpoint {
    address: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct ClustersResponse {
    #[serde(rename = "DescribeDBClustersResult")]
    describe_db_clusters_result: ClustersResult,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct ClustersResult {
    #[serde(rename = "DBClusters", default)]
    db_clusters: DbClusters,
    marker: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
struct DbClusters {
    #[serde(rename = "DBCluster", default)]
    values: Vec<ClusterXml>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct ClusterXml {
    #[serde(rename = "DBClusterIdentifier")]
    db_cluster_identifier: String,
    engine: Option<String>,
    engine_version: Option<String>,
    status: Option<String>,
    endpoint: Option<String>,
    #[serde(default)]
    availability_zones: AvailabilityZones,
}

#[derive(Debug, Default, Deserialize)]
struct AvailabilityZones {
    #[serde(rename = "AvailabilityZone", default)]
    values: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct Database {
    pub region: String,
    pub availability_zones: Vec<String>,
    pub identifier: String,
    pub kind: String,
    pub engine: Option<String>,
    pub class: Option<String>,
    pub status: Option<String>,
    pub endpoint: Option<String>,
}

pub fn describe_databases(client: &AwsClient, region: &str) -> Result<Vec<Database>> {
    let mut output = Vec::new();
    let mut marker: Option<String> = None;
    loop {
        let parameters = marker
            .iter()
            .map(|value| ("Marker".into(), value.clone()))
            .collect::<Vec<_>>();
        let response: ClustersResponse = call(client, region, "DescribeDBClusters", &parameters)?;
        output.extend(
            response
                .describe_db_clusters_result
                .db_clusters
                .values
                .into_iter()
                .map(|db| Database {
                    region: region.into(),
                    availability_zones: db.availability_zones.values,
                    identifier: db.db_cluster_identifier,
                    kind: "cluster".into(),
                    engine: combine(db.engine, db.engine_version),
                    class: None,
                    status: db.status,
                    endpoint: db.endpoint,
                }),
        );
        marker = response.describe_db_clusters_result.marker;
        if marker.is_none() {
            break;
        }
    }
    marker = None;
    loop {
        let parameters = marker
            .iter()
            .map(|value| ("Marker".into(), value.clone()))
            .collect::<Vec<_>>();
        let response: InstancesResponse = call(client, region, "DescribeDBInstances", &parameters)?;
        output.extend(
            response
                .describe_db_instances_result
                .db_instances
                .values
                .into_iter()
                .map(|db| Database {
                    region: region.into(),
                    availability_zones: db.availability_zone.into_iter().collect(),
                    identifier: db.db_instance_identifier,
                    kind: "instance".into(),
                    engine: combine(db.engine, db.engine_version),
                    class: db.db_instance_class,
                    status: db.db_instance_status,
                    endpoint: db.endpoint.and_then(|endpoint| endpoint.address),
                }),
        );
        marker = response.describe_db_instances_result.marker;
        if marker.is_none() {
            break;
        }
    }
    Ok(output)
}

fn combine(left: Option<String>, right: Option<String>) -> Option<String> {
    match (left, right) {
        (Some(left), Some(right)) => Some(format!("{left} {right}")),
        (Some(value), None) | (None, Some(value)) => Some(value),
        (None, None) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_rds_named_collection_elements() {
        let instances: InstancesResponse = quick_xml::de::from_str(
            r#"<DescribeDBInstancesResponse><DescribeDBInstancesResult><DBInstances><DBInstance><DBInstanceIdentifier>db-one</DBInstanceIdentifier><DBInstanceClass>db.t3.micro</DBInstanceClass><Engine>postgres</Engine><EngineVersion>16.3</EngineVersion><AvailabilityZone>us-east-1a</AvailabilityZone><Endpoint><Address>db.example</Address></Endpoint></DBInstance></DBInstances></DescribeDBInstancesResult></DescribeDBInstancesResponse>"#,
        )
        .unwrap();
        assert_eq!(
            instances.describe_db_instances_result.db_instances.values[0].db_instance_identifier,
            "db-one"
        );

        let clusters: ClustersResponse = quick_xml::de::from_str(
            r#"<DescribeDBClustersResponse><DescribeDBClustersResult><DBClusters><DBCluster><DBClusterIdentifier>cluster-one</DBClusterIdentifier><AvailabilityZones><AvailabilityZone>us-east-1a</AvailabilityZone><AvailabilityZone>us-east-1b</AvailabilityZone></AvailabilityZones></DBCluster></DBClusters></DescribeDBClustersResult></DescribeDBClustersResponse>"#,
        )
        .unwrap();
        assert_eq!(
            clusters.describe_db_clusters_result.db_clusters.values[0]
                .availability_zones
                .values,
            ["us-east-1a", "us-east-1b"]
        );
    }
}
