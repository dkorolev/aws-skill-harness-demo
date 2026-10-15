# AWS E2E Harness: ephemeral IPs vs. load balancers, taught by doing

A self-contained, agent-executable runbook that builds a tiny HTTP service on EC2, points DNS at it two different ways, breaks one of them on purpose, and cleans up after itself — proving every step with an explicit CHECK.

## The lesson

1. A Route53 **A record pointing at an instance's ephemeral public IP** works fine — until the instance is stopped and started. The IP changes, the record goes stale, and the name now points at an address you no longer own (a takeover-eligible dangling record in the making).
2. An **ALB with an alias record** survives the same stop/start with zero intervention: the alias tracks the load balancer, and the instance target re-registers itself because its *private* IP persists.

The harness demonstrates both, side by side, on the same instance.

## Contents

- **[E2E-HARNESS.md](E2E-HARNESS.md)** — the runbook. 15 phases (0–14) plus an emergency-teardown appendix. Written to be executed verbatim by a human or an AI agent: bounded waits everywhere, resolver-independent DNS checks, explicit pass/fail criteria including acceptable error codes.
- **[E2E-RUN-LOG-EXAMPLE.md](E2E-RUN-LOG-EXAMPLE.md)** — a real, complete run (all phases passed, no improvisation), sanitized: account ID, domain, IPs, and resource IDs replaced with documentation placeholders.
- **[counting_echo.py](counting_echo.py)** and **[counting-echo.service](counting-echo.service)** — the application the harness deploys: a stdlib-only HTTP echo server whose in-memory counter resets on restart (the evidence used in Phase 6), and the systemd unit that keeps it running. Phase 1 copies them from here into the work directory.
- **[AUDIT-HARNESS.md](AUDIT-HARNESS.md)** — a second, independent runbook: a phase-by-phase read-only audit of an AWS account that inventories everything with `awsw`, cross-checks the inventory against the bill, and classifies findings (wasted spend, dangling DNS, IAM hygiene) with an explicit severity model. It changes nothing. **[RUN-AWS-AUDIT.md](RUN-AWS-AUDIT.md)** is the one-prompt driver that runs the audit end to end with a strict read-only boundary and produces `AUDIT-REPORT-YYYY-MM-DD.md`.
- **[awsw](awsw)** — launcher for the supported inventory tool, a Rust crate in [cli/](cli/README.md) that speaks the AWS wire protocols directly (local SigV4 signing over an HTTPS-only rustls client) with no dependency on the `aws` CLI or an AWS SDK: identity, per-service inventory across all regions in parallel (EC2, EBS, EIPs, security groups, ELB + target health, ACM, RDS, ECS, VPC endpoints, SES, S3, CloudFront, Route53), month-to-date billing via Cost Explorer, and `discover` to run everything. Read-only except for the explicitly named Route 53 writes: `r53 --create`, `r53 --upsert-txt`, and `r53 --delete-txt`. Used by the harness for its verification sweeps; [AWSW.md](AWSW.md) is the entry point and [cli/ARCHITECTURE.md](cli/ARCHITECTURE.md) the design notes.

## Highlights

- **Security-group choreography**: SSH opens to the operator's /32 only, and is revoked — by lookup, and provably — the moment deployment ends.
- **Key hygiene**: the key pair is generated locally in a temp dir and only the public half is imported to AWS; nothing sensitive can land in git.
- **HTTP only, no EIP — both deliberate** and documented, so an executing agent doesn't "helpfully" fix the things the exercise depends on.
- **Teardown as a first-class phase**: ordered for dependency correctness (DNS → ALB → target group → instance → SGs → key), with a verification checklist that must return empty before the run counts as complete.
- Hardened over three full runs by incorporating executor feedback (resolver quirks, INSYNC waits, sshd readiness, API races).

## Before you run it

Substitute the placeholders (table at the top of the harness): your account ID, a Route53 zone you control, its authoritative NS, your default VPC. Use a scratch/sandbox account. A full run creates one t4g.micro, one ALB, two security groups, one key pair, and two DNS records — and deletes all of them. Total cost well under $0.25.

Nothing account-specific is committed here: `.gitignore` excludes run logs, audit reports, scratch state, and key material, and the harness keeps them local.
