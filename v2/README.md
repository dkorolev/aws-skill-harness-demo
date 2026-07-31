# awsw v2 — typed, manually signed AWS inventory

`awsw-v2` inventories an AWS account without the AWS CLI, an AWS SDK, botocore, Smithy-generated clients, or an implicit credential daemon. It is a Rust implementation of the repository's original account-reporting tool and constructs AWS request payloads and Signature Version 4 authorization headers inside the process.

The implementation is designed so that its credential and signing boundary can later move into a secure enclave or trusted execution environment (TEE). It is not itself an enclave today; see [Security model](#security-model) and [ARCHITECTURE.md](ARCHITECTURE.md).

## Quick start

From the repository root:

```sh
./awsw-v2 whoami
./awsw-v2 regions
./awsw-v2 --region us-west-1 ec2
./awsw-v2 --json discover
```

Or build and run the crate directly:

```sh
cargo build --manifest-path v2/Cargo.toml --release
v2/target/release/awsw-v2 whoami
```

The root launcher uses `cargo run --quiet`; Cargo rebuilds only when a source or dependency changed. A release deployment should install the release binary and does not need Cargo at runtime.

## What “manual” means

This project owns all AWS-specific client behavior:

- endpoint selection for the standard commercial AWS partition;
- AWS Query form payloads and dotted/list parameter names;
- AWS JSON 1.1 payloads and `X-Amz-Target` values;
- REST-JSON paths and query strings;
- REST-XML paths, query strings, response shapes, and create-zone XML;
- RFC 3986-compatible SigV4 URI and query encoding;
- canonical header construction, credential scope, derived signing keys, and `Authorization` headers;
- pagination tokens and aggregation;
- typed request and response structures for every supported operation.

It deliberately delegates non-AWS primitives to established general-purpose crates: rustls for TLS, HMAC/SHA-256 for cryptography, Serde for typed data, and quick-xml for safe XML escaping and decoding. Reimplementing those primitives would increase risk without increasing control of the AWS protocol.

`cargo tree` contains no AWS, botocore, boto, or Smithy client dependency.

## Commands

All inventory commands are read-only. Route 53 writes are exposed through explicitly named `r53` options.

| Command | AWS APIs | Scope | Result |
|---|---|---|---|
| `whoami` | STS, IAM | global | caller, account, aliases, selected region |
| `regions` | EC2 | global discovery | enabled regions |
| `iam` | IAM | global | summary, users, console access, keys, MFA, groups, policies |
| `ec2` | EC2 | all enabled regions | instances |
| `sgs` | EC2 | all enabled regions | security groups and rule counts |
| `volumes` | EC2 | all enabled regions | EBS volumes and attachments |
| `elb` | ELBv2 | all enabled regions | load balancers, target groups, health |
| `certs` | ACM | all enabled regions | certificate summaries |
| `rds` | RDS | all enabled regions | DB instances and clusters |
| `ecs` | ECS | all enabled regions | clusters and services |
| `eips` | EC2 | all enabled regions | Elastic IP associations |
| `snapshots` | EC2 | all enabled regions | self-owned EBS snapshots |
| `amis` | EC2 | all enabled regions | self-owned AMIs |
| `vpce` | EC2 | all enabled regions | VPC endpoints |
| `ses` | SES v2 | all enabled regions | email identities |
| `s3` | S3 | global | buckets and each bucket's region |
| `cloudfront` | CloudFront | global | distributions, aliases, origins |
| `r53` | Route 53 | global | zones and every record set |
| `billing` | Cost Explorer | global/us-east-1 | month-to-date service and region cost |
| `discover` | all APIs above | account-wide | one complete inventory |
| `r53 --create DOMAIN` | Route 53 | global, **write** | new public zone and delegation set |
| `r53 --upsert-txt NAME` | Route 53 | global, **write** | create or replace a TXT record set |
| `r53 --delete-txt NAME` | Route 53 | global, **write** | delete an exact TXT record set |

For a Let's Encrypt DNS-01 challenge, pass the TXT value without DNS quotes; the client adds the quotes Route 53 requires and safely serializes the request:

```sh
./awsw-v2 r53 --upsert-txt _acme-challenge.example.com \
  --zone-id Z1234567890 --value challenge-token --ttl 60

# After certificate issuance, use the same TTL and complete value set:
./awsw-v2 r53 --delete-txt _acme-challenge.example.com \
  --zone-id Z1234567890 --value challenge-token --ttl 60
```

Repeat `--value` for a multi-value TXT record set. Route 53 requires a DELETE to specify the record's exact current TTL and complete value set. Both commands return the Route 53 change ID and initial status; certificate tooling must still wait for DNS propagation before asking the ACME server to validate the proof.

Use `--region REGION` before or after a subcommand to restrict regional scans:

```sh
./awsw-v2 ec2 --region eu-north-1
./awsw-v2 --region eu-north-1 discover
```

Use `--json` for stable machine-readable output. Without it, v2 prints compact human-readable tables compatible in spirit with the original tool.

## Credentials and region selection

The credential provider checks, in order:

1. `AWS_ACCESS_KEY_ID` and `AWS_SECRET_ACCESS_KEY`, plus optional `AWS_SESSION_TOKEN`;
2. the profile named by `AWS_PROFILE` (default: `default`) in `AWS_SHARED_CREDENTIALS_FILE` (default: `~/.aws/credentials`).

Both access-key environment variables must be present together. The provider does not silently invoke SSO, `credential_process`, web identity, role assumption, ECS task credentials, EC2 instance metadata, or an external binary. That narrow behavior is intentional: an enclave caller should inject short-lived credentials through an explicit provider.

The request/signing region is selected from:

1. `--region`;
2. `AWS_REGION`;
3. `AWS_DEFAULT_REGION`;
4. `us-east-1`.

IAM, Route 53, CloudFront, S3 global discovery, and Cost Explorer use their documented global/us-east-1 signing behavior. Current endpoint construction supports the standard `amazonaws.com` commercial partition, not China or GovCloud.

## Protocol and transport behavior

Every request follows the same pipeline:

```text
typed operation
  -> protocol encoder
  -> method + endpoint + path + query + headers + body
  -> local SigV4 canonical request and HMAC chain
  -> rustls HTTPS request
  -> typed protocol decoder
  -> inventory/report model
```

Signed requests reject redirects. This prevents an `Authorization` header or session token from being automatically forwarded to another host. Requests have a 120-second timeout. AWS error status, code, message, and request ID are reported without logging request headers or credential material.

Regional scans execute concurrently through a bounded Rayon worker pool. Errors are isolated per region and printed to stderr so an unsupported service in one region does not erase inventory from the other regions. Query, JSON, and REST list operations follow their service-specific continuation tokens.

## Security model

### Protected by the current design

- No credential is placed in a URL, request body, command argument, or log.
- The secret access key and derived SigV4 keys are wrapped in zero-on-drop storage.
- `Credentials` has a redacted `Debug` implementation.
- XML request values are serialized and escaped, not interpolated into XML.
- HTTPS-only transport and disabled redirects narrow exfiltration paths.
- Typed operations prevent accidentally sending one service's shape to another service endpoint.
- Write operations are separately named Route 53 sub-options.

### Not protected yet

The normal host process can still inspect process memory and controls:

- the Rust binary and loaded libraries;
- environment variables and optional credentials file;
- wall clock, DNS, routing, and network availability;
- the system/root certificate set bundled through rustls dependencies;
- stdout/stderr and process lifecycle;
- the root signing key while the software signer is active.

There is no remote attestation, measured boot, sealed storage, anti-rollback state, trusted time source, or enclave-specific TLS channel. Do not describe this build as confidential computing merely because its interfaces anticipate that migration.

## TEE migration seam

`CredentialProvider` is the current source seam. The stronger end state is a `Signer` interface that accepts the canonical request scope and digest and returns an authorization result without releasing the root secret. The service modules and protocol encoders should remain outside that change.

A production migration should:

1. cross-compile a deterministic, minimized binary for the selected TEE;
2. replace file/environment credentials with an attested short-lived session;
3. move SigV4 key derivation behind the enclave boundary;
4. bind credential issuance to code measurement and policy;
5. establish trusted time or tightly bounded signed-time input;
6. pin or attest endpoint/DNS policy as required by the threat model;
7. produce an auditable egress allowlist;
8. zeroize request-header copies after transmission where the enclave runtime and HTTP stack permit it;
9. document rollback, renewal, revocation, and failure behavior.

See [ARCHITECTURE.md](ARCHITECTURE.md) for module-level details and design decisions.

## Development and verification

Offline verification:

```sh
cargo fmt --manifest-path v2/Cargo.toml -- --check
cargo test --manifest-path v2/Cargo.toml
cargo clippy --manifest-path v2/Cargo.toml --all-targets -- -D warnings
cargo build --manifest-path v2/Cargo.toml --release
```

Read-only live smoke tests:

```sh
./awsw-v2 --json whoami
./awsw-v2 --json regions
./awsw-v2 --region eu-north-1 --json discover > inventory.json
jq empty inventory.json
```

Do not automate `r53 --create` as part of a smoke test. It creates a billable, persistent account resource and is intentionally excluded from live tests; its payload construction and escaping are tested offline.

## Repository migration status

- `./awsw-v2` launches this Rust crate.
- `../awsw-v1-obsolete` is the old AWS CLI-backed implementation.

The obsolete v1 source remains temporarily useful as a behavioral reference. New features and fixes belong in this Rust crate.
