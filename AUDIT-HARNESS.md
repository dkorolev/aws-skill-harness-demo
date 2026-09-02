# Audit Harness: full read-only AWS account audit

A phase-by-phase runbook that inventories everything in an AWS account, cross-checks the inventory against the bill, and classifies findings — wasted spend, dangling DNS, IAM hygiene — with an explicit severity model. Companion tool: [`awsw-v2`](awsw-v2), a Rust client that constructs and signs AWS HTTPS payloads without the AWS CLI or an AWS SDK. Every inventory command in this harness is read-only; **the audit changes nothing**. Remediation is a separate, separately-authorized step.

**Executor contract**: run phases in order; each ends with what to RECORD. Write findings into `AUDIT-REPORT-YYYY-MM-DD.md` as you go — inventory tables first, findings after, suggested (not executed) remediation last. Never paste credentials, key material, or secret values into the report.

## Phase 0 — identity and scope

```sh
./awsw-v2 whoami
./awsw-v2 regions
```

RECORD: account ID, caller ARN, default region, region count.

**Immediate findings at this phase:**
- Caller ARN ends in `:root` → **finding, high**: root access keys in use. API keys bypass MFA even when console MFA is enabled. Remediation: create an IAM admin user, then deactivate and delete the root keys.
- No default region configured → note it; per-region tooling must pass `--region` explicitly.

## Phase 1 — billing first (the completeness oracle)

Run billing *before* the inventory: the bill is the one list that cannot miss a resource. Every service charging money must show up in the inventory sweep, and every populated region must be explainable.

```sh
./awsw-v2 billing         # month-to-date, by service and by region (us-east-1 Cost Explorer)
```

RECORD: cost by service, cost by region, total.

**Findings at this phase:**
- Cost Explorer disabled → **finding, medium**: no programmatic spend visibility. One console click to enable; data appears ~24h later.
- Any service in the bill you did not expect → mark it: the inventory sweep (Phase 2) must locate the resources behind it, or the audit is incomplete.
- Marketplace line items (e.g. hosted databases) — often the dominant cost; they will NOT appear in the resource inventory. Account for them here.

## Phase 2 — full inventory sweep

```sh
./awsw-v2 discover        # everything, all regions, in parallel
```

Or piecemeal: `ec2`, `volumes`, `snapshots`, `amis`, `eips`, `sgs`, `elb`, `certs`, `rds`, `ecs`, `vpce`, `ses`, `s3`, `cloudfront`, `r53` — each takes `--region` to narrow and `--json` for machine-readable output.

RECORD: counts per service per region; full tables in the report appendix.

**Cross-check against Phase 1**: each billed service maps to found resources (or a marketplace subscription). Each region with resources maps to expected activity. Anything unmatched in either direction is a gap — name it in the report.

## Phase 3 — compute & storage waste

From the Phase 2 tables, flag:

| Pattern | Cost while idle | Severity |
|---|---|---|
| Unassociated Elastic IP | ~$3.60/mo each ($0.005/hr) | waste |
| Unattached (`available`) EBS volume | $0.08–0.12/GiB-mo | waste |
| Stopped instance | its EBS storage only | waste (small) — but see below |
| Self-owned snapshots/AMIs no longer needed | $0.05/GiB-mo (snapshot) | waste (small) |

- Stopped instances deserve a decision, not just a note: terminate (with `DeleteOnTermination` volumes they free storage too), snapshot-then- terminate, or keep with a reason. Check `disableApiTermination` and volume `DeleteOnTermination` flags *before* recommending termination.
- Instances with very old short-form IDs (`i-xxxxxxxx`) or previous- generation types (t1/m1/t2) are usually forgotten — flag for review.
- Running instances that **no DNS record points at** (cross-reference Phase 5's owned-IP map): flag "what is this serving?".

## Phase 4 — TLS certificates

From `./awsw-v2 certs`:

- `EXPIRED` certificates → **finding**: delete (they cannot be renewed; re-issue instead). Verify `InUse: false` first — deletion fails otherwise, which is itself a finding (something is serving an expired cert).
- `ISSUED` but `InUse: false` → note; unused certs are free but are a renewal-failure risk if their validation records are gone.

## Phase 5 — DNS audit (the tiered dangling-record model)

This is the highest-signal part of the audit. Build two sets, then classify every A/CNAME/alias record in every hosted zone.

**Set 1 — IPs the account owns**: all instance public IPs + all EIPs (from Phase 2). **Set 2 — live resource DNS names**: every ELB DNS name, CloudFront domain, etc.

Classify each record:

| Tier | Points at | Risk | Action |
|---|---|---|---|
| **1** | An AWS-pool IP NOT in Set 1 | **Subdomain takeover** — anyone launching instances in that region can be handed the IP and silently serve content on your name | Delete or repoint **urgently** |
| **2** | A deleted resource *by name* (ELB/CloudFront DNS not in Set 2) | Low — ELB DNS names embed a random suffix, takeover impractical | Cleanup |
| **3** | A resource you own but that is dead (e.g. an idle EIP, a stopped instance) | None yet — but becomes Tier 1 the moment you release the resource | Fix records **before** releasing the resource |
| — | External (non-AWS) IPs/hosts | Unverifiable from AWS | Confirm out of band; assume intentional |

**The sequencing rule** (worth stating in every report): when releasing an EIP or terminating an instance that DNS points at, **delete or repoint the records first, then release**. The reverse order manufactures Tier 1 records.

Also flag: `external-dns` ownership TXT leftovers from dead clusters, stale ACM/LetsEncrypt validation records (`_acme-challenge`, comodoca/ `acm-validations` CNAMEs), and zones with no A records at all (parked or broken?).

## Phase 6 — IAM hygiene

```sh
./awsw-v2 iam
./awsw-v2 --json iam   # full per-user keys, MFA devices, groups, and policies
```

**Findings, in severity order:**
1. Any user with **admin-equivalent policy + console password + no MFA + stale last-use** → highest-severity finding in most audits. Immediate remediation: `aws iam delete-login-profile --user-name <u>` (revokes the password; keeps the user object for forensics).
2. Root access keys present (`RootKeys: 1`) → rotate to an IAM user.
3. Root console MFA disabled (`MFA: 0`) → enable.
4. Stale users/keys (unused > 90 days) → deactivate.

## Phase 7 — security groups

From `./awsw-v2 sgs` plus rule detail (the v2 JSON output preserves the summarized rule counts):

- `0.0.0.0/0` on 22/3389 attached to **running** instances → finding (severity is a policy call — flag it, let the owner decide).
- Default SGs with real inbound rules → finding: default groups should be empty; move rules to purpose-named groups.
- Unattached SGs across many regions are noise, not risk — count them, don't itemize.

## Phase 8 — the report

Assemble `AUDIT-REPORT-YYYY-MM-DD.md`:

1. **Identity** (and the root-keys finding if applicable).
2. **Billing** month-to-date, by service and region.
3. **Inventory** tables per service.
4. **Findings**, ordered: IAM (Phase 6) → Tier 1 DNS (Phase 5) → waste (Phase 3, with $/month) → certs (Phase 4) → SGs (Phase 7) → cross-check gaps (Phase 2).
5. **Suggested remediation**, explicitly marked NOT PERFORMED, with the sequencing rule spelled out for anything DNS-adjacent.

An audit is complete when every billed dollar, every region, and every DNS record is either explained or listed as a finding — not before.
