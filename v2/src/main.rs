use std::{
    env,
    fmt::Display,
    time::{SystemTime, UNIX_EPOCH},
};

use awsw_v2::{AwsClient, CredentialProvider, EnvironmentProvider, Result, services};
use clap::{Parser, Subcommand};
use rayon::prelude::*;
use serde::Serialize;

#[derive(Debug, Parser)]
#[command(
    version,
    about = "AWS inventory using manually built and signed HTTPS payloads"
)]
struct Cli {
    #[arg(long, global = true, help = "Emit machine-readable JSON")]
    json: bool,
    #[arg(long, global = true, help = "Restrict regional scans to one region")]
    region: Option<String>,
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Show the authenticated caller and AWS account.
    Whoami,
    /// List enabled commercial AWS regions.
    Regions,
    /// Audit IAM account summary, users, keys, MFA, groups, and policies.
    Iam,
    /// Inventory EC2 instances.
    Ec2,
    /// Inventory EC2 security groups and rule counts.
    Sgs,
    /// Inventory EBS volumes and attachments.
    Volumes,
    /// Inventory ELBv2 load balancers, target groups, and target health.
    Elb,
    /// Inventory ACM certificates.
    Certs,
    /// Inventory RDS instances and clusters.
    Rds,
    /// Inventory ECS clusters and services.
    Ecs,
    /// Inventory Elastic IP addresses and associations.
    Eips,
    /// Inventory self-owned EBS snapshots.
    Snapshots,
    /// Inventory self-owned AMIs.
    Amis,
    /// Inventory VPC endpoints.
    Vpce,
    /// Inventory SES v2 identities.
    Ses,
    /// Inventory S3 buckets and resolve their regions.
    S3,
    /// Inventory CloudFront distributions.
    Cloudfront,
    /// Inventory Route 53 zones/records, or create one public zone.
    R53 {
        #[arg(long, value_name = "DOMAIN")]
        create: Option<String>,
        #[arg(long, default_value = "")]
        comment: String,
    },
    /// Report month-to-date Cost Explorer totals.
    Billing,
    /// Run every inventory command and combine the results.
    Discover,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "PascalCase")]
struct Identity {
    account: String,
    arn: String,
    user_id: String,
    account_aliases: Vec<String>,
    region: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "PascalCase")]
struct ElbInventory {
    load_balancers: Vec<services::elb::LoadBalancer>,
    target_groups: Vec<services::elb::TargetGroup>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "PascalCase")]
struct Inventory {
    identity: Identity,
    iam: Option<services::iam::IamAudit>,
    billing: Option<services::cost_explorer::CostReport>,
    instances: Vec<services::ec2::Instance>,
    volumes: Vec<services::ec2::Volume>,
    snapshots: Vec<services::ec2::Snapshot>,
    images: Vec<services::ec2::Image>,
    addresses: Vec<services::ec2::Address>,
    security_groups: Vec<services::ec2::SecurityGroup>,
    load_balancers: Vec<services::elb::LoadBalancer>,
    target_groups: Vec<services::elb::TargetGroup>,
    certificates: Vec<services::acm::Certificate>,
    databases: Vec<services::rds::Database>,
    ecs_services: Vec<services::ecs::Service>,
    vpc_endpoints: Vec<services::ec2::VpcEndpoint>,
    ses_identities: Vec<services::ses::Identity>,
    buckets: Vec<services::s3::Bucket>,
    distributions: Vec<services::cloudfront::Distribution>,
    hosted_zones: Vec<services::route53::HostedZone>,
}

fn main() {
    if let Err(error) = run() {
        eprintln!("awsw-v2: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let cli = Cli::parse();
    let default_region = cli
        .region
        .clone()
        .or_else(|| env::var("AWS_REGION").ok())
        .or_else(|| env::var("AWS_DEFAULT_REGION").ok())
        .unwrap_or_else(|| "us-east-1".into());
    let client = AwsClient::new(EnvironmentProvider::default().provide()?)?;
    let scan_regions = resolve_regions(&client, &default_region, cli.region.as_deref())?;

    match cli.command {
        Command::Whoami => render_identity(&identity(&client, &default_region)?, cli.json)?,
        Command::Regions => {
            let mut regions = services::ec2::describe_regions(&client, &default_region)?;
            regions.sort_by(|a, b| a.region_name.cmp(&b.region_name));
            emit(
                cli.json,
                &regions,
                &format!("enabled regions ({})", regions.len()),
                &["REGION"],
                regions
                    .iter()
                    .map(|r| vec![r.region_name.clone()])
                    .collect(),
            )?;
        }
        Command::Iam => render_iam(services::iam::audit(&client)?, cli.json)?,
        Command::Ec2 => render_instances(
            regional(&client, &scan_regions, services::ec2::describe_instances),
            cli.json,
        )?,
        Command::Sgs => render_sgs(
            regional(
                &client,
                &scan_regions,
                services::ec2::describe_security_groups,
            ),
            cli.json,
        )?,
        Command::Volumes => render_volumes(
            regional(&client, &scan_regions, services::ec2::describe_volumes),
            cli.json,
        )?,
        Command::Elb => {
            let inventory = ElbInventory {
                load_balancers: regional(
                    &client,
                    &scan_regions,
                    services::elb::describe_load_balancers,
                ),
                target_groups: regional(
                    &client,
                    &scan_regions,
                    services::elb::describe_target_groups,
                ),
            };
            render_elb(inventory, cli.json)?;
        }
        Command::Certs => render_certs(
            regional(&client, &scan_regions, services::acm::list_certificates),
            cli.json,
        )?,
        Command::Rds => render_rds(
            regional(&client, &scan_regions, services::rds::describe_databases),
            cli.json,
        )?,
        Command::Ecs => render_ecs(
            regional(&client, &scan_regions, services::ecs::list_services),
            cli.json,
        )?,
        Command::Eips => render_eips(
            regional(&client, &scan_regions, services::ec2::describe_addresses),
            cli.json,
        )?,
        Command::Snapshots => render_snapshots(
            regional(&client, &scan_regions, services::ec2::describe_snapshots),
            cli.json,
        )?,
        Command::Amis => render_images(
            regional(&client, &scan_regions, services::ec2::describe_images),
            cli.json,
        )?,
        Command::Vpce => render_vpce(
            regional(
                &client,
                &scan_regions,
                services::ec2::describe_vpc_endpoints,
            ),
            cli.json,
        )?,
        Command::Ses => render_ses(
            regional(&client, &scan_regions, services::ses::list_email_identities),
            cli.json,
        )?,
        Command::S3 => render_s3(services::s3::list_buckets(&client)?, cli.json)?,
        Command::Cloudfront => {
            render_cloudfront(services::cloudfront::list_distributions(&client)?, cli.json)?
        }
        Command::R53 { create, comment } => {
            if let Some(domain) = create {
                let caller = format!(
                    "awsw-v2-{}-{}",
                    domain.trim_end_matches('.'),
                    SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_millis()
                );
                let comment = if comment.is_empty() {
                    format!(
                        "created by awsw-v2 {}",
                        chrono::Utc::now().format("%Y-%m-%d")
                    )
                } else {
                    comment
                };
                let response = services::route53::create_hosted_zone(
                    &client,
                    domain.trim_end_matches('.'),
                    &comment,
                    &caller,
                )?;
                if cli.json {
                    print_json(&response)?;
                } else {
                    section(&format!(
                        "created hosted zone {}",
                        response.hosted_zone.name
                    ));
                    println!(
                        "  Id          : {}",
                        response
                            .hosted_zone
                            .id
                            .rsplit('/')
                            .next()
                            .unwrap_or(&response.hosted_zone.id)
                    );
                    println!("  NameServers :");
                    for server in response.delegation_set.name_servers.values {
                        println!("    {server}");
                    }
                }
            } else {
                render_route53(services::route53::list_hosted_zones(&client)?, cli.json)?;
            }
        }
        Command::Billing => {
            render_billing(services::cost_explorer::month_to_date(&client)?, cli.json)?
        }
        Command::Discover => {
            let inventory = discover(&client, &default_region, &scan_regions);
            if cli.json {
                print_json(&inventory)?;
            } else {
                render_discover(inventory)?;
            }
        }
    }
    Ok(())
}

fn identity(client: &AwsClient, region: &str) -> Result<Identity> {
    let identity = services::sts::get_caller_identity(client, region)?;
    Ok(Identity {
        account: identity.account,
        arn: identity.arn,
        user_id: identity.user_id,
        account_aliases: services::iam::list_account_aliases(client)?,
        region: region.into(),
    })
}

fn resolve_regions(client: &AwsClient, default: &str, only: Option<&str>) -> Result<Vec<String>> {
    Ok(match only {
        Some(region) => vec![region.into()],
        None => services::ec2::describe_regions(client, default)?
            .into_iter()
            .map(|r| r.region_name)
            .collect(),
    })
}

fn regional<T: Send, F>(client: &AwsClient, regions: &[String], operation: F) -> Vec<T>
where
    F: Fn(&AwsClient, &str) -> Result<Vec<T>> + Sync,
{
    regions
        .par_iter()
        .flat_map(|region| match operation(client, region) {
            Ok(values) => values,
            Err(error) => {
                eprintln!("  [error] {region}: {error}");
                Vec::new()
            }
        })
        .collect()
}

fn discover(client: &AwsClient, default_region: &str, regions: &[String]) -> Inventory {
    let identity = identity(client, default_region).unwrap_or_else(|error| {
        eprintln!("  [error] identity: {error}");
        Identity {
            account: "-".into(),
            arn: "-".into(),
            user_id: "-".into(),
            account_aliases: Vec::new(),
            region: default_region.into(),
        }
    });
    let billing = services::cost_explorer::month_to_date(client)
        .map_err(|error| eprintln!("  [error] billing: {error}"))
        .ok();
    Inventory {
        identity,
        iam: services::iam::audit(client)
            .map_err(|error| eprintln!("  [error] IAM: {error}"))
            .ok(),
        billing,
        instances: regional(client, regions, services::ec2::describe_instances),
        volumes: regional(client, regions, services::ec2::describe_volumes),
        snapshots: regional(client, regions, services::ec2::describe_snapshots),
        images: regional(client, regions, services::ec2::describe_images),
        addresses: regional(client, regions, services::ec2::describe_addresses),
        security_groups: regional(client, regions, services::ec2::describe_security_groups),
        load_balancers: regional(client, regions, services::elb::describe_load_balancers),
        target_groups: regional(client, regions, services::elb::describe_target_groups),
        certificates: regional(client, regions, services::acm::list_certificates),
        databases: regional(client, regions, services::rds::describe_databases),
        ecs_services: regional(client, regions, services::ecs::list_services),
        vpc_endpoints: regional(client, regions, services::ec2::describe_vpc_endpoints),
        ses_identities: regional(client, regions, services::ses::list_email_identities),
        buckets: services::s3::list_buckets(client).unwrap_or_else(|error| {
            eprintln!("  [error] s3: {error}");
            Vec::new()
        }),
        distributions: services::cloudfront::list_distributions(client).unwrap_or_else(|error| {
            eprintln!("  [error] cloudfront: {error}");
            Vec::new()
        }),
        hosted_zones: services::route53::list_hosted_zones(client).unwrap_or_else(|error| {
            eprintln!("  [error] route53: {error}");
            Vec::new()
        }),
    }
}

fn render_identity(value: &Identity, json: bool) -> Result<()> {
    if json {
        return print_json(value);
    }
    section("whoami");
    let alias = if value.account_aliases.is_empty() {
        String::new()
    } else {
        format!(" ({})", value.account_aliases.join(", "))
    };
    println!("  Account : {}{alias}", value.account);
    println!("  ARN     : {}", value.arn);
    println!("  UserId  : {}", value.user_id);
    println!("  Region  : {}", value.region);
    Ok(())
}

fn render_iam(value: services::iam::IamAudit, json: bool) -> Result<()> {
    if json {
        return print_json(&value);
    }
    section("IAM account summary");
    let mut summary = value
        .summary
        .into_iter()
        .map(|(key, value)| vec![key, value.to_string()])
        .collect::<Vec<_>>();
    summary.sort_by(|left, right| left[0].cmp(&right[0]));
    table(&["KEY", "VALUE"], summary);
    section(&format!("IAM users ({})", value.users.len()));
    table(
        &[
            "USER",
            "CONSOLE",
            "MFA",
            "KEYS",
            "GROUPS",
            "PASSWORD LAST USED",
        ],
        value
            .users
            .iter()
            .map(|user| {
                vec![
                    user.user_name.clone(),
                    if user.console_password {
                        "yes".into()
                    } else {
                        "no".into()
                    },
                    user.mfa_devices.len().to_string(),
                    user.access_keys.len().to_string(),
                    user.groups
                        .iter()
                        .map(|group| group.group_name.clone())
                        .collect::<Vec<_>>()
                        .join(","),
                    d(&user.password_last_used),
                ]
            })
            .collect(),
    );
    for user in value.users {
        for group in user.groups {
            if !group.attached_policies.is_empty() {
                println!(
                    "  {} / {} policies: {}",
                    user.user_name,
                    group.group_name,
                    group
                        .attached_policies
                        .into_iter()
                        .map(|policy| policy.policy_name)
                        .collect::<Vec<_>>()
                        .join(", ")
                );
            }
        }
    }
    Ok(())
}

fn render_instances(v: Vec<services::ec2::Instance>, j: bool) -> Result<()> {
    let rows = v
        .iter()
        .map(|x| {
            vec![
                x.region.clone(),
                d(&x.availability_zone),
                x.instance_id.clone(),
                d(&x.name),
                d(&x.instance_type),
                d(&x.state),
                d(&x.public_ip),
                d(&x.private_ip),
            ]
        })
        .collect();
    emit(
        j,
        &v,
        &format!("EC2 instances ({})", v.len()),
        &[
            "REGION",
            "AZ",
            "ID",
            "NAME",
            "TYPE",
            "STATE",
            "PUBLIC IP",
            "PRIVATE IP",
        ],
        rows,
    )
}
fn render_sgs(v: Vec<services::ec2::SecurityGroup>, j: bool) -> Result<()> {
    let rows = v
        .iter()
        .map(|x| {
            vec![
                x.region.clone(),
                x.group_id.clone(),
                d(&x.name),
                d(&x.vpc_id),
                x.ingress_rules.to_string(),
                x.egress_rules.to_string(),
                d(&x.description),
            ]
        })
        .collect();
    emit(
        j,
        &v,
        &format!("security groups ({})", v.len()),
        &["REGION", "ID", "NAME", "VPC", "IN", "OUT", "DESCRIPTION"],
        rows,
    )
}
fn render_volumes(v: Vec<services::ec2::Volume>, j: bool) -> Result<()> {
    let rows = v
        .iter()
        .map(|x| {
            vec![
                x.region.clone(),
                d(&x.availability_zone),
                x.volume_id.clone(),
                d(&x.name),
                d(&x.size_gib),
                d(&x.volume_type),
                d(&x.state),
                if x.attached_to.is_empty() {
                    "-".into()
                } else {
                    x.attached_to.join(",")
                },
            ]
        })
        .collect();
    emit(
        j,
        &v,
        &format!("EBS volumes ({})", v.len()),
        &[
            "REGION",
            "AZ",
            "ID",
            "NAME",
            "GiB",
            "TYPE",
            "STATE",
            "ATTACHED TO",
        ],
        rows,
    )
}
fn render_eips(v: Vec<services::ec2::Address>, j: bool) -> Result<()> {
    let rows = v
        .iter()
        .map(|x| {
            vec![
                x.region.clone(),
                d(&x.public_ip),
                d(&x.name),
                d(&x.instance_id),
                d(&x.network_interface_id),
                if x.associated {
                    "yes".into()
                } else {
                    "NO (billed!)".into()
                },
            ]
        })
        .collect();
    emit(
        j,
        &v,
        &format!("Elastic IPs ({})", v.len()),
        &[
            "REGION",
            "PUBLIC IP",
            "NAME",
            "INSTANCE",
            "ENI",
            "ASSOCIATED",
        ],
        rows,
    )
}
fn render_snapshots(v: Vec<services::ec2::Snapshot>, j: bool) -> Result<()> {
    let rows = v
        .iter()
        .map(|x| {
            vec![
                x.region.clone(),
                x.snapshot_id.clone(),
                d(&x.name),
                d(&x.size_gib),
                d(&x.state),
                short(&x.started),
                d(&x.description),
            ]
        })
        .collect();
    emit(
        j,
        &v,
        &format!("EBS snapshots, self-owned ({})", v.len()),
        &[
            "REGION",
            "ID",
            "NAME",
            "GiB",
            "STATE",
            "STARTED",
            "DESCRIPTION",
        ],
        rows,
    )
}
fn render_images(v: Vec<services::ec2::Image>, j: bool) -> Result<()> {
    let rows = v
        .iter()
        .map(|x| {
            vec![
                x.region.clone(),
                x.image_id.clone(),
                d(&x.name),
                short(&x.created),
                d(&x.state),
                d(&x.public),
            ]
        })
        .collect();
    emit(
        j,
        &v,
        &format!("AMIs, self-owned ({})", v.len()),
        &["REGION", "ID", "NAME", "CREATED", "STATE", "PUBLIC"],
        rows,
    )
}
fn render_vpce(v: Vec<services::ec2::VpcEndpoint>, j: bool) -> Result<()> {
    let rows = v
        .iter()
        .map(|x| {
            vec![
                x.region.clone(),
                x.vpc_endpoint_id.clone(),
                d(&x.name),
                d(&x.endpoint_type),
                d(&x.service),
                d(&x.vpc_id),
                d(&x.state),
            ]
        })
        .collect();
    emit(
        j,
        &v,
        &format!("VPC endpoints ({})", v.len()),
        &["REGION", "ID", "NAME", "TYPE", "SERVICE", "VPC", "STATE"],
        rows,
    )
}
fn render_certs(v: Vec<services::acm::Certificate>, j: bool) -> Result<()> {
    let rows = v
        .iter()
        .map(|x| {
            vec![
                x.region.clone(),
                d(&x.domain_name),
                d(&x.status),
                d(&x.certificate_type),
                d(&x.in_use),
                d(&x.not_after),
                x.subject_alternative_names.join(","),
            ]
        })
        .collect();
    emit(
        j,
        &v,
        &format!("ACM certificates ({})", v.len()),
        &[
            "REGION", "DOMAIN", "STATUS", "TYPE", "IN USE", "EXPIRES", "SANs",
        ],
        rows,
    )
}
fn render_rds(v: Vec<services::rds::Database>, j: bool) -> Result<()> {
    let rows = v
        .iter()
        .map(|x| {
            vec![
                x.region.clone(),
                x.availability_zones.join(","),
                x.identifier.clone(),
                x.kind.clone(),
                d(&x.engine),
                d(&x.class),
                d(&x.status),
                d(&x.endpoint),
            ]
        })
        .collect();
    emit(
        j,
        &v,
        &format!("RDS databases ({})", v.len()),
        &[
            "REGION", "AZ", "ID", "KIND", "ENGINE", "CLASS", "STATUS", "ENDPOINT",
        ],
        rows,
    )
}
fn render_ecs(v: Vec<services::ecs::Service>, j: bool) -> Result<()> {
    let rows = v
        .iter()
        .map(|x| {
            vec![
                x.region.clone(),
                x.cluster.clone(),
                d(&x.service),
                d(&x.status),
                format!("{}/{}", d(&x.running), d(&x.desired)),
                if x.launch.is_empty() {
                    "-".into()
                } else {
                    x.launch.join(",")
                },
            ]
        })
        .collect();
    emit(
        j,
        &v,
        &format!("ECS services ({})", v.len()),
        &[
            "REGION", "CLUSTER", "SERVICE", "STATUS", "RUN/WANT", "LAUNCH",
        ],
        rows,
    )
}
fn render_ses(v: Vec<services::ses::Identity>, j: bool) -> Result<()> {
    let rows = v
        .iter()
        .map(|x| {
            vec![
                x.region.clone(),
                x.identity.clone(),
                d(&x.identity_type),
                d(&x.sending_enabled),
                d(&x.verification_status),
            ]
        })
        .collect();
    emit(
        j,
        &v,
        &format!("SES identities ({})", v.len()),
        &["REGION", "IDENTITY", "TYPE", "SENDING", "STATUS"],
        rows,
    )
}
fn render_s3(v: Vec<services::s3::Bucket>, j: bool) -> Result<()> {
    let rows = v
        .iter()
        .map(|x| vec![x.region.clone(), x.name.clone(), d(&x.created)])
        .collect();
    emit(
        j,
        &v,
        &format!("S3 buckets ({})", v.len()),
        &["REGION", "NAME", "CREATED"],
        rows,
    )
}
fn render_cloudfront(v: Vec<services::cloudfront::Distribution>, j: bool) -> Result<()> {
    let rows = v
        .iter()
        .map(|x| {
            vec![
                x.id.clone(),
                d(&x.domain_name),
                x.aliases.join(","),
                x.origins.join(","),
                d(&x.status),
                d(&x.enabled),
            ]
        })
        .collect();
    emit(
        j,
        &v,
        &format!("CloudFront distributions ({})", v.len()),
        &["ID", "DOMAIN", "ALIASES", "ORIGINS", "STATUS", "ENABLED"],
        rows,
    )
}

fn render_elb(v: ElbInventory, json: bool) -> Result<()> {
    if json {
        return print_json(&v);
    }
    section(&format!("load balancers ({})", v.load_balancers.len()));
    table(
        &["REGION", "NAME", "TYPE", "SCHEME", "STATE", "AZs", "DNS"],
        v.load_balancers
            .iter()
            .map(|x| {
                vec![
                    x.region.clone(),
                    x.name.clone(),
                    d(&x.load_balancer_type),
                    d(&x.scheme),
                    d(&x.state),
                    x.availability_zones.join(","),
                    d(&x.dns_name),
                ]
            })
            .collect(),
    );
    section(&format!("target groups ({})", v.target_groups.len()));
    table(
        &["REGION", "NAME", "PROTO", "TARGET TYPE", "HEALTH", "LBs"],
        v.target_groups
            .iter()
            .map(|x| {
                vec![
                    x.region.clone(),
                    x.name.clone(),
                    format!("{}:{}", d(&x.protocol), d(&x.port)),
                    d(&x.target_type),
                    format!("{}/{} healthy", x.healthy_targets, x.total_targets),
                    x.load_balancers.to_string(),
                ]
            })
            .collect(),
    );
    Ok(())
}

fn render_route53(v: Vec<services::route53::HostedZone>, json: bool) -> Result<()> {
    if json {
        return print_json(&v);
    }
    section(&format!("Route53 hosted zones ({})", v.len()));
    for zone in v {
        println!(
            "\n  {} ({}, {}, {} records)",
            zone.name,
            zone.id,
            if zone.private { "private" } else { "public" },
            zone.records.len()
        );
        table(
            &["NAME", "TYPE", "TTL", "VALUE"],
            zone.records
                .iter()
                .map(|r| {
                    vec![
                        r.name.clone(),
                        r.record_type.clone(),
                        d(&r.ttl),
                        r.alias_target
                            .as_ref()
                            .map(|a| format!("ALIAS -> {}", a.dns_name))
                            .unwrap_or_else(|| r.values.join(" ")),
                    ]
                })
                .collect(),
        );
    }
    Ok(())
}

fn render_billing(v: services::cost_explorer::CostReport, json: bool) -> Result<()> {
    if json {
        return print_json(&v);
    }
    section(&format!(
        "billing, month to date ({} .. {})",
        v.start, v.end
    ));
    billing_table("By service", &v.by_service);
    billing_table("By region", &v.by_region);
    Ok(())
}
fn billing_table(title: &str, v: &services::cost_explorer::Response) {
    let mut rows = Vec::new();
    let mut total = 0.0;
    for time in &v.results_by_time {
        for group in &time.groups {
            let amount = group
                .metrics
                .unblended_cost
                .amount
                .parse::<f64>()
                .unwrap_or(0.0);
            total += amount;
            if amount.abs() >= 0.005 {
                rows.push(vec![
                    group.keys.first().cloned().unwrap_or_default(),
                    format!("{amount:.2}"),
                ]);
            }
        }
    }
    rows.sort_by(|a, b| b[1].partial_cmp(&a[1]).unwrap_or(std::cmp::Ordering::Equal));
    rows.push(vec!["TOTAL".into(), format!("{total:.2}")]);
    println!("\n  {title}:");
    table(&["DIMENSION", "USD"], rows);
}

fn render_discover(v: Inventory) -> Result<()> {
    render_identity(&v.identity, false)?;
    if let Some(x) = v.iam {
        render_iam(x, false)?;
    }
    if let Some(x) = v.billing {
        render_billing(x, false)?;
    }
    render_instances(v.instances, false)?;
    render_volumes(v.volumes, false)?;
    render_snapshots(v.snapshots, false)?;
    render_images(v.images, false)?;
    render_eips(v.addresses, false)?;
    render_sgs(v.security_groups, false)?;
    render_elb(
        ElbInventory {
            load_balancers: v.load_balancers,
            target_groups: v.target_groups,
        },
        false,
    )?;
    render_certs(v.certificates, false)?;
    render_rds(v.databases, false)?;
    render_ecs(v.ecs_services, false)?;
    render_vpce(v.vpc_endpoints, false)?;
    render_ses(v.ses_identities, false)?;
    render_s3(v.buckets, false)?;
    render_cloudfront(v.distributions, false)?;
    render_route53(v.hosted_zones, false)
}

fn emit<T: Serialize>(
    json: bool,
    value: &T,
    title: &str,
    headers: &[&str],
    rows: Vec<Vec<String>>,
) -> Result<()> {
    if json {
        print_json(value)
    } else {
        section(title);
        table(headers, rows);
        Ok(())
    }
}
fn print_json(value: &impl Serialize) -> Result<()> {
    println!("{}", serde_json::to_string_pretty(value)?);
    Ok(())
}
fn d<T: Display>(value: &Option<T>) -> String {
    value
        .as_ref()
        .map(ToString::to_string)
        .unwrap_or_else(|| "-".into())
}
fn short(value: &Option<String>) -> String {
    value
        .as_ref()
        .map(|x| x.chars().take(10).collect())
        .unwrap_or_else(|| "-".into())
}
fn section(title: &str) {
    println!(
        "\n=== {title} {}",
        "=".repeat(60usize.saturating_sub(title.len()))
    );
}
fn table(headers: &[&str], rows: Vec<Vec<String>>) {
    if rows.is_empty() {
        println!("  (none)");
        return;
    }
    let widths = headers
        .iter()
        .enumerate()
        .map(|(i, h)| {
            rows.iter()
                .map(|r| r[i].len())
                .max()
                .unwrap_or(0)
                .max(h.len())
        })
        .collect::<Vec<_>>();
    println!(
        "  {}",
        headers
            .iter()
            .enumerate()
            .map(|(i, h)| format!("{h:<w$}", w = widths[i]))
            .collect::<Vec<_>>()
            .join("  ")
    );
    println!(
        "  {}",
        widths
            .iter()
            .map(|w| "-".repeat(*w))
            .collect::<Vec<_>>()
            .join("  ")
    );
    for row in rows {
        println!(
            "  {}",
            row.iter()
                .enumerate()
                .map(|(i, v)| format!("{v:<w$}", w = widths[i]))
                .collect::<Vec<_>>()
                .join("  ")
        );
    }
}
