use std::env;

use awsw_v2::{AwsClient, CredentialProvider, EnvironmentProvider, Result, services};
use clap::{Parser, Subcommand};
use serde::Serialize;

#[derive(Debug, Parser)]
#[command(
    version,
    about = "AWS inventory using manually built and signed HTTPS payloads"
)]
struct Cli {
    /// Emit machine-readable JSON.
    #[arg(long, global = true)]
    json: bool,

    /// Signing region, or restrict a regional scan to this region.
    #[arg(long, global = true)]
    region: Option<String>,

    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Print caller identity, account alias, and selected region.
    Whoami,
    /// List enabled AWS regions.
    Regions,
    /// List EC2 instances in one region or every enabled region.
    Ec2,
}

#[derive(Serialize)]
#[serde(rename_all = "PascalCase")]
struct WhoamiOutput {
    account: String,
    arn: String,
    user_id: String,
    account_aliases: Vec<String>,
    region: String,
}

fn main() {
    if let Err(error) = run() {
        eprintln!("awsw-v2: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let cli = Cli::parse();
    let configured_region = cli
        .region
        .clone()
        .or_else(|| env::var("AWS_REGION").ok())
        .or_else(|| env::var("AWS_DEFAULT_REGION").ok())
        .unwrap_or_else(|| "us-east-1".into());
    let credentials = EnvironmentProvider::default().provide()?;
    let client = AwsClient::new(credentials)?;

    match cli.command {
        Command::Whoami => {
            let identity = services::sts::get_caller_identity(&client, &configured_region)?;
            let aliases = services::iam::list_account_aliases(&client)?;
            let output = WhoamiOutput {
                account: identity.account,
                arn: identity.arn,
                user_id: identity.user_id,
                account_aliases: aliases,
                region: configured_region,
            };
            if cli.json {
                print_json(&output)?;
            } else {
                println!("\n=== whoami =====================================================");
                let alias = if output.account_aliases.is_empty() {
                    String::new()
                } else {
                    format!(" ({})", output.account_aliases.join(", "))
                };
                println!("  Account : {}{}", output.account, alias);
                println!("  ARN     : {}", output.arn);
                println!("  UserId  : {}", output.user_id);
                println!("  Region  : {}", output.region);
            }
        }
        Command::Regions => {
            let mut regions = services::ec2::describe_regions(&client, &configured_region)?;
            regions.sort_by(|left, right| left.region_name.cmp(&right.region_name));
            if cli.json {
                print_json(&regions)?;
            } else {
                println!(
                    "\n=== enabled regions ({}) =======================================",
                    regions.len()
                );
                for region in regions {
                    println!("  {}", region.region_name);
                }
            }
        }
        Command::Ec2 => {
            let regions = if cli.region.is_some() {
                vec![configured_region]
            } else {
                services::ec2::describe_regions(&client, &configured_region)?
                    .into_iter()
                    .map(|region| region.region_name)
                    .collect()
            };
            let mut instances = Vec::new();
            for region in regions {
                instances.extend(services::ec2::describe_instances(&client, &region)?);
            }
            if cli.json {
                print_json(&instances)?;
            } else {
                println!(
                    "\n=== EC2 instances ({}) =========================================",
                    instances.len()
                );
                table(
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
                    instances
                        .into_iter()
                        .map(|instance| {
                            vec![
                                instance.region,
                                display(instance.availability_zone),
                                instance.instance_id,
                                display(instance.name),
                                display(instance.instance_type),
                                display(instance.state),
                                display(instance.public_ip),
                                display(instance.private_ip),
                            ]
                        })
                        .collect(),
                );
            }
        }
    }
    Ok(())
}

fn print_json(value: &impl Serialize) -> Result<()> {
    println!("{}", serde_json::to_string_pretty(value)?);
    Ok(())
}

fn display(value: Option<String>) -> String {
    value.unwrap_or_else(|| "-".into())
}

fn table(headers: &[&str], rows: Vec<Vec<String>>) {
    if rows.is_empty() {
        println!("  (none)");
        return;
    }
    let widths = headers
        .iter()
        .enumerate()
        .map(|(index, header)| {
            rows.iter()
                .map(|row| row[index].len())
                .max()
                .unwrap_or(0)
                .max(header.len())
        })
        .collect::<Vec<_>>();
    println!(
        "  {}",
        headers
            .iter()
            .enumerate()
            .map(|(index, header)| format!("{header:<width$}", width = widths[index]))
            .collect::<Vec<_>>()
            .join("  ")
    );
    println!(
        "  {}",
        widths
            .iter()
            .map(|width| "-".repeat(*width))
            .collect::<Vec<_>>()
            .join("  ")
    );
    for row in rows {
        println!(
            "  {}",
            row.iter()
                .enumerate()
                .map(|(index, value)| format!("{value:<width$}", width = widths[index]))
                .collect::<Vec<_>>()
                .join("  ")
        );
    }
}
