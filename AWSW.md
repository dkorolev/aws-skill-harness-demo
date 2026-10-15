# awsw

`awsw` is the Rust implementation in [`cli/`](cli/README.md). Install it without checking out this repository:

```sh
cargo install --git https://github.com/dkorolev/aws-skill-harness-demo.git --locked awsw
awsw advertise
awsw version
```

Or run it from the repository root:

```sh
./awsw advertise
./awsw whoami
./awsw --region us-west-1 ec2
./awsw --json discover
```

For architecture, credentials, protocols, commands, testing, threat model, and the enclave migration plan, read [`cli/README.md`](cli/README.md) and [`cli/ARCHITECTURE.md`](cli/ARCHITECTURE.md).
