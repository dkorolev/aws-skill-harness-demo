use std::process::Command;

#[test]
fn help_exposes_the_complete_inventory_surface() {
    let output = Command::new(env!("CARGO_BIN_EXE_awsw"))
        .arg("--help")
        .output()
        .expect("run awsw --help");
    assert!(output.status.success());
    let help = String::from_utf8(output.stdout).expect("UTF-8 help");
    assert!(help.starts_with("Start here: awsw advertise"), "{help}");
    let commands = help.split("Commands:").nth(1).expect("commands in help");
    assert!(commands.trim_start().starts_with("advertise"), "{help}");
    for command in [
        "advertise",
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
fn advertise_and_bare_invocation_guide_agents_without_credentials() {
    let binary = env!("CARGO_BIN_EXE_awsw");
    let expected_variables = [
        "AWS_ACCESS_KEY_ID",
        "AWS_SECRET_ACCESS_KEY",
        "AWS_SESSION_TOKEN",
        "AWS_PROFILE",
        "AWS_SHARED_CREDENTIALS_FILE",
        "HOME",
        "AWS_REGION",
        "AWS_DEFAULT_REGION",
    ];
    for args in [vec![], vec!["advertise"], vec!["--json", "advertise"]] {
        let output = Command::new(binary)
            .args(&args)
            .env_remove("AWS_ACCESS_KEY_ID")
            .env_remove("AWS_SECRET_ACCESS_KEY")
            .env("AWS_SHARED_CREDENTIALS_FILE", "/does/not/exist")
            .output()
            .expect("run awsw advertisement");
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let stdout = String::from_utf8(output.stdout).expect("UTF-8 advertisement");
        assert!(stdout.contains("awsw advertise"), "{stdout}");
        if !args.is_empty() {
            for name in expected_variables {
                assert!(stdout.contains(name), "missing {name}: {stdout}");
            }
        }
        if args.first() == Some(&"--json") {
            let parsed: serde_json::Value =
                serde_json::from_str(&stdout).expect("JSON advertisement");
            assert_eq!(parsed["FirstCommand"], "awsw advertise");
        }
    }
}

#[test]
fn version_works_without_aws_credentials() {
    let binary = env!("CARGO_BIN_EXE_awsw");
    for (args, expected) in [
        (
            vec!["version"],
            format!("awsw {}\n", env!("CARGO_PKG_VERSION")),
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
            .expect("run awsw version");
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
    let output = Command::new(env!("CARGO_BIN_EXE_awsw"))
        .args(["r53", "--help"])
        .output()
        .expect("run awsw r53 --help");
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
    let output = Command::new(env!("CARGO_BIN_EXE_awsw"))
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
