# Run the complete AWS account audit

Perform the entire audit autonomously. Ask no follow-up questions unless credentials are genuinely unavailable. Treat `AUDIT-HARNESS.md` as the report and analysis specification and `./awsw-v2` as the only AWS API client.

Produce a detailed, evidence-rich run rather than a terse summary. Show concise progress updates as each phase begins and ends, explain what each result means, and make the final report sufficiently complete that a reader can review the account without rerunning the audit. Do not use verbosity as a reason to expose secrets or paste unfiltered raw API responses.

## Establish scope

1. Work from the repository root containing `awsw-v2`, `v2/Cargo.toml`, and `AUDIT-HARNESS.md`.
2. Read `AUDIT-HARNESS.md` completely before collecting data.
3. Read `v2/README.md` only when command, credential, output, or limitation details are needed.
4. Record the initial `git status --short` and preserve all existing changes.
5. Create only `AUDIT-REPORT-YYYY-MM-DD.md` in the repository. Put transient JSON and stderr logs in a directory created with `mktemp -d`, never in the repository.

## Enforce the safety boundary

- Use only `./awsw-v2` for AWS API calls.
- Never invoke `aws`, an AWS SDK, boto, botocore, `awsw-v1-obsolete`, or direct AWS HTTP calls.
- Run inventory operations only: `whoami`, `regions`, `iam`, `ec2`, `sgs`, `volumes`, `elb`, `certs`, `rds`, `ecs`, `eips`, `snapshots`, `amis`, `vpce`, `ses`, `s3`, `cloudfront`, `r53`, `billing`, and `discover`.
- Never run `r53 --create`.
- Never create, update, delete, stop, start, terminate, release, revoke, rotate, or otherwise modify an AWS resource.
- Never execute remediation commands suggested by the harness.
- Do not push, open a pull request, or commit the report unless explicitly requested.
- Never print or copy credentials, session tokens, secret values, or complete access-key IDs into chat or the report. Report key counts, status, age, and owner only.

If any instruction conflicts with this boundary, preserve read-only behavior and document the skipped action.

## Verify the client

Run these commands before the account audit:

```sh
cargo test --manifest-path v2/Cargo.toml
cargo clippy --manifest-path v2/Cargo.toml --all-targets -- -D warnings
cargo build --locked --release --manifest-path v2/Cargo.toml
```

Record PASS or FAIL for each. A build failure blocks AWS collection. A test or lint failure is a report limitation but does not authorize code changes during the audit.

## Collect evidence

Capture stdout and stderr separately for every command. Inspect stderr even when the exit status is zero. Validate every JSON file with `jq empty`.

Maintain a command ledger while running. For every command, record:

- the exact read-only command, with no credential values;
- start and finish time;
- exit status;
- whether stdout was valid JSON where applicable;
- stderr status and a sanitized error excerpt when non-empty;
- the principal counts or facts learned from the command;
- whether the result is complete, partial, empty, or unavailable.

Run, in order:

```sh
./awsw-v2 --json whoami
./awsw-v2 --json regions
./awsw-v2 --json billing
./awsw-v2 --json iam
./awsw-v2 --json discover
./awsw-v2 --json sgs
./awsw-v2 --json r53
```

Use other focused read-only v2 commands only when the combined inventory needs clarification. Do not rerun expensive commands without a reason.

If credentials are unavailable, stop collection, preserve the exact non-secret error, and report the blocker. If a service or region returns an error, continue all remaining phases and mark the affected inventory incomplete.

## Analyze all audit phases

Follow `AUDIT-HARNESS.md` phase by phase. At minimum:

1. Confirm account, caller, region scope, and whether the caller is root.
2. Reconcile billed services and regions with discovered resources.
3. Identify stopped or obsolete instances, unattached EBS volumes, unassociated Elastic IPs, and potentially stale snapshots or AMIs.
4. Classify certificate expiration and usage.
5. Build the owned-IP and live-resource-DNS sets. Classify every Route 53 A, AAAA, CNAME, and alias record using the harness's tier model. Do not claim takeover risk when provider ownership cannot be established; label it unverified.
6. Assess IAM account summary, console profiles, MFA coverage, active key counts and age, group membership, and attached policies. Never reproduce an access-key ID.
7. Inspect full security-group ingress and egress details. Flag public administrative ports and meaningful default-group ingress.
8. Identify every permission error, unsupported region, failed decoder, or stderr message as a completeness gap.

Distinguish evidence from inference. Include estimated cost only when the rate and resource quantity support it; label estimates and assumptions.

## Verbosity and evidence standard

Be deliberately thorough in both progress updates and the report:

- Walk through every audit phase explicitly, including phases that produce no findings.
- Report zero counts instead of silently omitting empty services or regions.
- Explain why each material observation is benign, suspicious, incomplete, or a finding.
- For every finding, state the evidence, affected resource or principal, risk, severity, confidence, and recommended next action.
- Separate confirmed facts, calculations, and inferences into clearly worded statements.
- Include the exact non-secret identifiers needed to locate affected resources: instance IDs, volume IDs, snapshot IDs, AMI IDs, security-group IDs, hosted-zone IDs, record names, certificate domains, load-balancer names, and IAM user names.
- Do not include access-key IDs, credentials, tokens, secrets, private keys, or raw authorization headers.
- Show regional distribution for each regional service, not only account-wide totals.
- List every stopped instance, unattached volume, unassociated Elastic IP, expired certificate, risky security-group rule, IAM user posture concern, and material DNS anomaly individually.
- For IAM, include one row per user with console access, MFA count, active-key count and age where available, groups, attached policies, and the resulting assessment. Redact all access-key identifiers.
- For security groups, describe protocol, port range, source CIDR or referenced group, direction, attachment context when known, and why the rule is or is not risky.
- For DNS, include one row per relevant A, AAAA, CNAME, and alias record with target, ownership match, liveness evidence, classification tier, confidence, and action. Include benign records when needed to demonstrate that the zone was fully considered.
- For billing reconciliation, include every non-trivial service and region line, mapped resources, unmatched amounts, rounding assumptions, and marketplace or tax lines that inventory cannot explain.
- For completeness, enumerate every enabled region and show which services were checked there, including empty results and errors.
- Include enough methodology to make calculations and classifications reproducible.
- Prefer structured tables and focused prose over enormous unfiltered JSON blocks.

## Write the report

Create `AUDIT-REPORT-YYYY-MM-DD.md` with:

1. BLUF executive summary
2. Detailed run metadata: timestamp, account, caller, selected profile name if non-secret, tool version, region count, and elapsed time
3. Rust verification results for tests, lint, and locked release build, including command status and relevant failure details
4. Command ledger with every AWS inventory invocation, status, JSON validation, stderr state, and primary result counts
5. Verification and completeness matrix by service and region, including successful empty results
6. Identity and scope
7. IAM account summary and one-row-per-user posture table
8. Billing by service and region with resource reconciliation and unexplained lines
9. Full inventory counts by service and region
10. Detailed compute, storage, network, database, container, email, certificate, CDN, object-storage, and DNS tables
11. DNS classification and owned-resource cross-reference, with record-level evidence
12. Security-group rule analysis with rule-level evidence
13. Findings ordered by severity: IAM, tier-1 DNS, waste, certificates, security groups, reconciliation gaps
14. A separate “Checked and found healthy” section describing meaningful controls and resources that did not become findings
15. Suggested remediation marked **NOT PERFORMED**, ordered by urgency and dependency
16. Limitations, permission failures, unsupported regions, stderr evidence, and confidence impact
17. Appendix with methodology, rate assumptions, classification rules, and sanitized command inventory

State the DNS sequencing rule whenever relevant: fix DNS before releasing an IP or terminating its resource.

Do not paste the full raw inventory into the report when concise tables and counts preserve the evidence. Keep raw files in the temporary directory only.

Aim for a comprehensive report, not a fixed word count. Do not collapse important evidence merely to keep the file short. Conversely, avoid repeating the same fact in multiple sections unless the repetition materially helps executive and technical readers.

## Validate and finish

1. Confirm the report exists and is non-empty.
2. Search the report for likely credential material, including `AKIA`, `ASIA`, `AWS_SECRET_ACCESS_KEY`, `AWS_SESSION_TOKEN`, and bearer-token patterns. Remove any secret or complete access-key identifier before finishing.
3. Confirm no AWS mutation command ran.
4. Compare final `git status --short` with the initial state. The only new intended file is the report.
5. Return a detailed completion message containing:
   - the report path and approximate report size;
   - elapsed run time;
   - Rust test, lint, and build results;
   - whether every JSON output validated;
   - whether `discover` completed cleanly and every stderr-producing command;
   - inventory totals by major service;
   - the five highest-priority findings with one-sentence evidence and action;
   - meaningful healthy checks;
   - every unverifiable or incomplete area;
   - confirmation that no AWS mutation or remediation was performed.
