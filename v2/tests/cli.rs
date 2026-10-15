use std::process::Command;

#[test]
fn help_exposes_the_complete_inventory_surface() {
    let output = Command::new(env!("CARGO_BIN_EXE_awsw-v2"))
        .arg("--help")
        .output()
        .expect("run awsw-v2 --help");
    assert!(output.status.success());
    let help = String::from_utf8(output.stdout).expect("UTF-8 help");
    for command in [
        "version",
        "whoami",
        "regions",
        "iam",
        "ec2",
        "sgs",
        "volumes",
        "elb",
        "certs",
        "rds",
        "ecs",
        "eips",
        "snapshots",
        "amis",
        "vpce",
        "ses",
        "s3",
        "cloudfront",
        "r53",
        "billing",
        "discover",
    ] {
        assert!(help.contains(command), "missing {command} from help");
    }
}

#[test]
fn version_works_without_aws_credentials() {
    let binary = env!("CARGO_BIN_EXE_awsw-v2");
    for (args, expected) in [
        (
            vec!["version"],
            format!("awsw-v2 {}\n", env!("CARGO_PKG_VERSION")),
        ),
        (
            vec!["--json", "version"],
            format!("{{\n  \"Version\": \"{}\"\n}}\n", env!("CARGO_PKG_VERSION")),
        ),
    ] {
        let output = Command::new(binary)
            .args(args)
            .env_remove("AWS_ACCESS_KEY_ID")
            .env_remove("AWS_SECRET_ACCESS_KEY")
            .env("AWS_SHARED_CREDENTIALS_FILE", "/does/not/exist")
            .output()
            .expect("run awsw-v2 version");
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            String::from_utf8(output.stdout).expect("UTF-8 version"),
            expected
        );
    }
}

#[test]
fn route53_help_exposes_txt_record_changes() {
    let output = Command::new(env!("CARGO_BIN_EXE_awsw-v2"))
        .args(["r53", "--help"])
        .output()
        .expect("run awsw-v2 r53 --help");
    assert!(output.status.success());
    let help = String::from_utf8(output.stdout).expect("UTF-8 help");
    for option in [
        "--upsert-txt",
        "--delete-txt",
        "--zone-id",
        "--value",
        "--ttl",
    ] {
        assert!(help.contains(option), "missing {option} from Route 53 help");
    }
}

#[test]
fn route53_txt_change_validates_required_arguments_before_credentials() {
    let output = Command::new(env!("CARGO_BIN_EXE_awsw-v2"))
        .args(["r53", "--upsert-txt", "_acme-challenge.example.com"])
        .env_remove("AWS_ACCESS_KEY_ID")
        .env_remove("AWS_SECRET_ACCESS_KEY")
        .env("AWS_SHARED_CREDENTIALS_FILE", "/does/not/exist")
        .output()
        .expect("run invalid Route 53 change");
    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).expect("UTF-8 stderr");
    assert!(stderr.contains("--zone-id is required"), "{stderr}");
    assert!(!stderr.contains("credentials unavailable"), "{stderr}");
}
