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
