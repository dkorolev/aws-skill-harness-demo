# awsw v2 (Rust)

`awsw-v2` is the Rust successor to the repository's Python/AWS CLI wrapper. It constructs AWS service payloads and Signature Version 4 authorization headers itself and sends them over HTTPS. It does **not** use the AWS CLI, an AWS SDK, botocore, or an AWS credential helper.

The current vertical slice supports:

- `whoami` through the typed STS and IAM Query APIs;
- `regions` through the typed EC2 Query API;
- `ec2` for typed instance inventory in one or all enabled regions;
- `--json` and `--region`, matching the original command-line shape.

The transport and protocol foundation is intended to support the remaining v1 inventory services without changing the security boundary.

## Build and run

```sh
cargo build --manifest-path v2/Cargo.toml --release
v2/target/release/awsw-v2 whoami
v2/target/release/awsw-v2 --region us-west-1 ec2
v2/target/release/awsw-v2 --json regions
```

Credentials are read from `AWS_ACCESS_KEY_ID`, `AWS_SECRET_ACCESS_KEY`, and optionally `AWS_SESSION_TOKEN`. If those are absent, the selected `AWS_PROFILE` is read directly from `AWS_SHARED_CREDENTIALS_FILE` or `~/.aws/credentials`. No credential process, SSO command, container endpoint, or instance metadata service is invoked implicitly.

## Trust boundary

The `CredentialProvider` trait is the enclave migration seam. Today, `EnvironmentProvider` returns zero-on-drop signing material. A future TEE build can replace it with an attested provider or a signer that never releases the root key. Service modules only produce typed requests and do not know where credentials came from.

General-purpose crates provide rustls HTTPS, HMAC/SHA-256, JSON, and XML. This project manually implements the AWS-specific Query/JSON/REST payload rules and SigV4 canonicalization; it intentionally does not attempt to implement TLS or cryptographic primitives.

## Test

```sh
cargo test --manifest-path v2/Cargo.toml
cargo clippy --manifest-path v2/Cargo.toml --all-targets -- -D warnings
```

Tests are offline and do not require credentials. Live commands are read-only.
