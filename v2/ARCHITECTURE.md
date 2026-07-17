# awsw v2 architecture and enclave design notes

This document describes the internal boundaries of the Rust implementation, why they exist, and how they map to a future trusted execution environment.

## Goals

1. Inventory the same account resources as the original `awsw` program.
2. Have no runtime or build-time dependency on an AWS client implementation.
3. Make AWS payload construction and SigV4 behavior locally auditable.
4. Keep service responses strongly typed at the network boundary.
5. Separate credential acquisition, signing, transport, decoding, and reports.
6. Preserve a credible path to an enclave-held signing secret.

## Non-goals

- Reimplement TLS, HMAC, SHA-256, JSON, or XML parsers.
- Become a general replacement for the AWS CLI.
- Support arbitrary service operations or user-supplied endpoints.
- Discover credentials through every mechanism supported by AWS tooling.
- Claim current host execution is confidential or remotely attestable.
- Support GovCloud and China partitions in the initial endpoint resolver.

## Module map

```text
src/main.rs
  CLI, regional fan-out, tables, JSON reports, discover orchestration

src/services/*.rs
  typed operation request/response models and service pagination

src/protocols/query.rs
  AWS Query form bodies and XML response decoding

src/protocols/json.rs
  AWS JSON 1.1 bodies, targets, and response decoding

src/protocols/rest_json.rs
  signed REST paths/query strings with JSON decoding

src/protocols/rest_xml.rs
  signed REST paths/query strings, XML decoding, escaped XML writes

src/client.rs
  HTTPS-only transport, redirect policy, request IDs, error boundary

src/sigv4.rs
  canonical URI/query/headers, request digest, HMAC key derivation

src/credentials.rs
  credential value type and the replaceable provider boundary

src/error.rs
  credential, protocol, transport, and AWS service error taxonomy
```

Dependency direction runs downward in this diagram. Protocol and service code does not read environment variables, inspect credential files, or construct an HTTP client. Credential code does not know which services are called.

## Request lifecycle

For an EC2 `DescribeInstances` page:

1. `services::ec2` selects API version `2016-11-15` and creates typed Query parameters.
2. `protocols::query` adds `Action` and `Version`, RFC 3986-encodes fields, and constructs a deterministic form body.
3. `client::Request` carries the signing service, region, endpoint, path, query, signed headers, and bytes independently.
4. `client::AwsClient` adds `Host`, `X-Amz-Date`, and an optional session token.
5. `sigv4` hashes the body, canonicalizes the request, derives the date, region, service, and `aws4_request` HMAC keys, and returns `Authorization`.
6. reqwest sends the request through its rustls backend with HTTPS-only and redirects disabled.
7. The response status and request ID are checked before decoding.
8. quick-xml decodes the page into the EC2 response type.
9. The service follows `nextToken`, maps wire types into report types, and returns the accumulated inventory.
10. `main` combines regions and renders a table or serialized report.

JSON and REST services use the same steps with their appropriate encoder.

## Wire protocol inventory

### AWS Query

Query operations use a POST body with deterministic percent-encoded form fields. The action and version are part of the signed body.

- STS `GetCallerIdentity`, version `2011-06-15`
- IAM identity and audit operations, version `2010-05-08`
- EC2 describe operations, version `2016-11-15`
- ELBv2 describe operations, version `2015-12-01`, signing name `elasticloadbalancing`
- RDS describe operations, version `2014-10-31`

XML capitalization is service-specific. For example, EC2 uses lower-camel container names while RDS preserves the acronym in `DBInstances`. Explicit Serde renames capture those differences rather than normalizing through an untyped map.

### AWS JSON 1.1

These operations POST compact JSON with an `application/x-amz-json-1.1` content type and signed `X-Amz-Target`:

- ACM `CertificateManager.ListCertificates`
- ECS `AmazonEC2ContainerServiceV20141113.*`
- Cost Explorer `AWSInsightsIndexService.GetCostAndUsage`

### REST-JSON

SES v2 lists email identities with a signed GET path and continuation token.

### REST-XML

- S3 lists buckets and resolves each bucket location.
- CloudFront lists distributions under the pinned API-version path.
- Route 53 lists zones and record sets under the `2013-04-01` path.
- Route 53 creation serializes a typed public-zone request. XML text and attributes are escaped by quick-xml.

## SigV4 invariants

The signer enforces or constructs these invariants:

- `Host` must be present in the signed header set.
- Header names are lowercase and sorted for canonicalization.
- Sequential ASCII whitespace in header values is collapsed.
- Query keys and values are independently encoded and sorted after encoding.
- Spaces are `%20`, never `+`.
- URI path segments preserve `/` separators while encoding segment content.
- The exact transmitted body bytes produce the payload digest.
- Session tokens are signed through `x-amz-security-token`.
- S3 requests include the signed `x-amz-content-sha256` header.
- The credential scope is date/region/service/`aws4_request`.
- The root-key prefix and all derived keys use zero-on-drop buffers.

The client uses a single timestamp for the date header and signature to avoid a second-boundary mismatch.

## Credential boundary

`CredentialProvider::provide` currently returns a `Credentials` object. It is small enough to replace without affecting services, but returning the secret is not the ideal enclave API.

The target design is conceptually:

```rust,ignore
trait Signer {
    fn access_key_id(&self) -> &str;
    fn session_token(&self) -> Option<&str>;
    fn sign(&self, scope: Scope, string_to_sign: &[u8]) -> Signature;
}
```

An enclave-backed signer can then retain the root secret internally. The host may still see the session access-key ID, token, request payload, and final signature; the threat model must decide whether payload confidentiality also requires moving serialization and TLS into the enclave.

## Concurrency and failure isolation

Regional list operations run in parallel. The result order follows the input region order, while individual service calls within a region preserve API page order. A regional error is reported and produces no values for that region, matching the inventory tool's best-effort nature. Global commands return an error directly because there is no other region from which to recover.

`discover` additionally isolates global service failures and continues, so a disabled Cost Explorer account does not prevent EC2 or DNS inventory.

This behavior is appropriate for discovery, but callers requiring proof of completeness must treat any stderr error as an incomplete report.

## Write-operation containment

Route 53 zone creation is the only write. It is not part of `discover`, has no alias shorter than `r53 --create DOMAIN`, and is not exercised in live tests. Its caller reference contains the normalized domain and current epoch milliseconds to make retries distinguishable. AWS remains the idempotency and conflict authority.

A stricter enclave deployment should compile writes behind a feature flag or remove them entirely from an audit-only binary.

## Supply chain and reproducibility

`Cargo.lock` is committed. Release builders should use:

```sh
cargo build --locked --release --manifest-path v2/Cargo.toml
```

For measured deployment, also pin the Rust toolchain, vendor crates, record source checksums, strip nondeterministic metadata where possible, and compare the final binary measurement in CI and the attestation policy.

## Test strategy

The offline suite covers:

- credential profile selection and debug redaction behavior;
- Query payload construction;
- query and path percent encoding;
- deterministic SigV4 structure and signed-header selection;
- compact EC2 XML without formatting assumptions;
- Route 53 typed XML serialization and escaping;
- CLI help exposing the complete command surface.

Live read-only verification covers each service protocol and a restricted `discover`. It is intentionally separate from unit tests because it requires real credentials and account permissions. No live test invokes zone creation.

Future fixtures should be captured as synthetic/minimized documents, not raw account responses, so repository tests do not retain account identifiers, resource names, DNS data, or billing information.
