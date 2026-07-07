# E2E Harness: counting echo server on EC2 + DNS + ALB, then full teardown

An end-to-end exercise for a scratch AWS account. It demonstrates, in order:
direct-to-instance serving, DNS pointing at an ephemeral IP (and why that
breaks), and an ALB as the fix — then tears everything down and proves it.

Revision 3, hardened by executor feedback from two prior full runs. Treat
this document as code: every command is meant to run verbatim, every wait is
an explicit bounded loop, and environmental assumptions (shell, resolver,
prior state) are checked or neutralized.

**Placeholders — substitute your own values before running:**

| Placeholder | Meaning |
|---|---|
| `111122223333` | your AWS account ID |
| `example.com` | a domain whose Route53 hosted zone you control |
| `Z123456789EXAMPLE` | that zone's hosted zone ID |
| `ns-123.awsdns-45.net` | one of that zone's authoritative name servers (`aws route53 get-hosted-zone`) |
| `vpc-0123456789abcdef0` | the default VPC in your chosen region |
| `us-west-1` | any region with ≥2 AZs works; adjust the AMI SSM parameter arch if needed |

**Executor contract**: run phases strictly in order. Every phase ends with a
CHECK; do not proceed until the CHECK passes. All created resources are tagged
`Project=simpletest` so a tag sweep can find strays.

**Scratch file**: record every created resource ID as you go — the teardown
consumes it. Format: shell-sourceable, `export KEY="value"` lines only,
values always double-quoted (unquoted values with spaces break `source`).
Full-line `#` comments are fine. Keep it in `$WORKDIR`; **never write it
anywhere under the repository tree** (`.gitignore` can guard the commit, but
the rule guards the leak). The teardown DELETE for `simpletest.example.com`
must use **`$IP1` — the stale value from Phase 3 — never `$IP2`**; freeze
`IP1` in the scratch file and do not overwrite it at Phase 11.

**Git / run-log policy**: at run start, create
`E2E-RUN-LOG-YYYY-MM-DD-HH-MM-SS.md` (timestamp = run start, local time) and
log the run there: one section per phase with the key outputs, resource IDs,
and the CHECK verdict. Commit **after each passing phase**, and the commit
must touch **only the run-log file** (message: `E2E run <timestamp>: phase N
passed`). Do not commit anything else during the run — not the app files
(they are already embedded in this harness) and **never** the scratch file or
key material (transient state, operator IP, private key). Commit the phase-N
log entry **before beginning phase N+1** — writing two phases and committing
once blurs the audit trail. If a phase fails, log the failure, commit the
failure entry, run the Appendix A emergency teardown, and stop.

## ⚠️ Ground rules

- **HTTP only, no HTTPS.** The service, the direct test, and the ALB listener
  are all plain HTTP (ports 8000 and 80). No TLS certificate is requested, no
  :443 listener exists, and browsers will (correctly) mark it "Not secure".
  This is deliberate: the exercise is about IPs/DNS/ALB mechanics, not TLS.
  Do not "helpfully" add an ACM cert or HTTPS listener.
- **No Elastic IP, deliberately.** The whole point of the stop/start phase is
  to observe the public IP changing and DNS breaking. Do not attach an EIP.
- **Region: `us-west-1`.** Hosted zone `example.com` = `Z123456789EXAMPLE`.
  Everything regional lives in the default VPC (`vpc-0123456789abcdef0`).
- **SSH lifecycle**: the instance security group starts with port 22 open
  (restricted to the operator's `/32`) for deployment; after the service is
  deployed and verified, all port-22 rules are **revoked by lookup**
  (Phase 8). Everything after that point must work without SSH. The key pair
  is generated locally in a temp dir and only the public half is sent to AWS.
- **Naming**: every resource is named `simpletest-*`.
- **Cost**: t4g.micro ≈ $0.008/hr, ALB ≈ $0.03/hr + trivial LCU, 8 GiB gp3
  ≈ $0.01/hr, public IPv4 ≈ $0.005/hr. A 2-hour run costs well under $0.25.
  The teardown (Phase 13–14) is what keeps it that way.

### DNS ground rules

The operator's local resolver is NOT trusted. Split DNS, VPNs, and Tailscale
(observed: a Tailscale resolver returning NXDOMAIN for subdomains of a
Route53-hosted zone while Route53 was INSYNC) can make correct records look
broken and vice versa. Therefore:

- **Every DNS assertion uses two probes**: the zone's authoritative server
  (`dig @ns-123.awsdns-45.net`) and a public resolver (`dig @8.8.8.8`).
  Authoritative proves Route53 state; 8.8.8.8 proves public propagation.
- **Every hostname HTTP check uses `curl --resolve name:port:ip`** with the
  IP taken from the authoritative dig — this tests the service and the Host
  header without depending on the operator's resolver. A bare-hostname curl
  may be run *in addition*, as a bonus check, but its failure is never a
  phase failure.
- **Every `change-resource-record-sets` call (UPSERT and DELETE alike) is
  followed by a bounded INSYNC wait.** Capture the change ID from the call
  itself — **never re-submit the change to discover it**:

  ```sh
  r53_wait() {   # usage: r53_wait "$CHANGE_ID"
    for i in $(seq 1 30); do
      STATUS=$(aws route53 get-change --id "$1" \
        --query 'ChangeInfo.Status' --output text)
      [ "$STATUS" = INSYNC ] && break
      sleep 2
    done
    echo "route53 change: $STATUS"; [ "$STATUS" = INSYNC ]
  }
  ```

  The phase FAILS if the loop exhausts without INSYNC. (The built-in
  `aws route53 wait resource-record-sets-changed --id "$CHANGE_ID"` is the
  lazy alternative, but it polls at 30 s intervals — slow for a TTL-60
  exercise.)
- **ALB alias records resolve to a set of IPs, and different resolvers
  return different subsets.** Never require the authoritative answer and the
  8.8.8.8 answer to match; each returning *at least one* A record is the
  assertion.

## Phase 0 — preflight

```sh
export AWS_DEFAULT_REGION=us-west-1
aws sts get-caller-identity            # expect Account 111122223333 (yours)
VPC=vpc-0123456789abcdef0
# ALB needs two subnets in different AZs; default VPC has one per AZ:
aws ec2 describe-subnets --filters Name=vpc-id,Values=$VPC \
  --query 'Subnets[].[SubnetId,AvailabilityZone]' --output text
MYIP=$(curl -s https://checkip.amazonaws.com)   # operator IP for the SSH rule
WORKDIR=$(mktemp -d)                   # holds key material + scratch file
```

**Leftover check** — a failed prior run can leave name collisions that block
Phases 2–3. Run all of these; the happy path is: the first two return nothing
(or only `terminated` instances), and the key-pair check **fails with
`InvalidKeyPair.NotFound` — that error is the desired outcome**, not a
problem:

```sh
aws ec2 describe-instances --filters Name=tag:Project,Values=simpletest \
  --query 'Reservations[].Instances[].[InstanceId,State.Name]' --output text
aws ec2 describe-security-groups \
  --filters Name=group-name,Values=simpletest-instance,simpletest-alb \
  --query 'SecurityGroups[].GroupId' --output text
aws ec2 describe-key-pairs --key-names simpletest-key 2>&1 | grep -q NotFound \
  && echo "key: clean" || echo "key: LEFTOVER EXISTS"
test ! -f e2e-scratch.txt \
  && echo "scratch: clean" || echo "scratch: STALE FILE IN REPO ROOT — delete it"
```

If anything other than `terminated` instances shows up, run the Appendix A
emergency teardown first, then restart Phase 0.

**CHECK**: correct account; at least 2 subnets in 2 distinct AZs (call them
`$SUBNET_A`, `$SUBNET_B`); `$MYIP` is a plausible public IPv4; `$WORKDIR`
exists; leftover check clean.

## Phase 1 — the application

`counting_echo.py` — stdlib only, listens on 0.0.0.0:8000. Each response
carries a monotonically increasing in-memory counter plus an echo of the
request. The counter resets on process restart — that is expected and is used
later as evidence of a restart.

```python
#!/usr/bin/env python3
"""Counting echo server: GET anything, get back a counter + echo."""
import itertools
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

counter = itertools.count(1)

class Handler(BaseHTTPRequestHandler):
    def do_GET(self):
        body = f"count={next(counter)} path={self.path} host={self.headers.get('Host')}\n"
        data = body.encode()
        self.send_response(200)
        self.send_header("Content-Type", "text/plain")
        self.send_header("Content-Length", str(len(data)))
        self.end_headers()
        self.wfile.write(data)
    def log_message(self, *a):
        pass

ThreadingHTTPServer(("0.0.0.0", 8000), Handler).serve_forever()
```

`counting-echo.service` — systemd unit so the service starts on boot and
restarts on crash (this is what "survives restart" means):

```ini
[Unit]
Description=Counting echo server (simpletest)
After=network.target

[Service]
ExecStart=/usr/bin/python3 /opt/counting_echo.py
Restart=always
User=ec2-user

[Install]
WantedBy=multi-user.target
```

Write both into `$WORKDIR` (not the repo — they are already embedded here).

**CHECK**: `python3 $WORKDIR/counting_echo.py` runs locally and
`curl localhost:8000/x` returns `count=1 path=/x ...`, second curl returns
`count=2 ...`. Kill it.

## Phase 2 — security groups

Two groups. The instance group opens SSH **only to the operator** and the app
port to the world (world access is needed for the direct-DNS test, and later
provides the clean "DNS points to a dead IP" failure rather than a confusing
SG block).

```sh
SG_INST=$(aws ec2 create-security-group --group-name simpletest-instance \
  --description "simpletest instance: temp SSH + HTTP 8000" --vpc-id $VPC \
  --tag-specifications 'ResourceType=security-group,Tags=[{Key=Project,Value=simpletest}]' \
  --query GroupId --output text)
aws ec2 authorize-security-group-ingress --group-id $SG_INST \
  --protocol tcp --port 22 --cidr $MYIP/32          # TEMPORARY — revoked in Phase 8
aws ec2 authorize-security-group-ingress --group-id $SG_INST \
  --protocol tcp --port 8000 --cidr 0.0.0.0/0       # plain HTTP, on purpose

SG_ALB=$(aws ec2 create-security-group --group-name simpletest-alb \
  --description "simpletest ALB: HTTP 80 from world" --vpc-id $VPC \
  --tag-specifications 'ResourceType=security-group,Tags=[{Key=Project,Value=simpletest}]' \
  --query GroupId --output text)
aws ec2 authorize-security-group-ingress --group-id $SG_ALB \
  --protocol tcp --port 80 --cidr 0.0.0.0/0         # plain HTTP, on purpose
```

**CHECK**: `aws ec2 describe-security-groups --group-ids $SG_INST $SG_ALB`
shows exactly the rules above and nothing more. Record both IDs in the
scratch file.

## Phase 3 — key pair and instance

The key is generated **locally** in `$WORKDIR`; only the public half is
imported to AWS (the private key never transits the AWS API, and lives
outside the repo so it cannot be committed by accident).

```sh
ssh-keygen -t ed25519 -N '' -f $WORKDIR/simpletest-key
aws ec2 import-key-pair --key-name simpletest-key \
  --public-key-material fileb://$WORKDIR/simpletest-key.pub

AMI=$(aws ssm get-parameter \
  --name /aws/service/ami-amazon-linux-latest/al2023-ami-kernel-default-arm64 \
  --query Parameter.Value --output text)

IID=$(aws ec2 run-instances --image-id $AMI --instance-type t4g.micro \
  --key-name simpletest-key --security-group-ids $SG_INST --subnet-id $SUBNET_A \
  --tag-specifications 'ResourceType=instance,Tags=[{Key=Name,Value=simpletest},{Key=Project,Value=simpletest}]' \
  --query 'Instances[0].InstanceId' --output text)
aws ec2 wait instance-running --instance-ids $IID
IP1=$(aws ec2 describe-instances --instance-ids $IID \
  --query 'Reservations[0].Instances[0].PublicIpAddress' --output text)
```

**CHECK**: instance state `running`; `$IP1` set. Record `$IID` and `$IP1`
(plus the private IP) in the scratch file; **`IP1` is now frozen** — it is
the value the teardown's DNS DELETE will need.

## Phase 4 — deploy over SSH

Do not stuff the ssh command into a scalar variable (`SSH="ssh ..."` then
`$SSH cmd` breaks under zsh word-splitting). Use a function:

```sh
ssh_inst() {
  ssh -o StrictHostKeyChecking=accept-new -i "$WORKDIR/simpletest-key" \
    "ec2-user@$IP1" "$@"
}
# instance-running means the VM booted, not that sshd is up — bounded
# readiness poll before the first scp (cap 5 min, FAIL on exhaustion):
for i in $(seq 1 30); do
  ssh -o StrictHostKeyChecking=accept-new -o ConnectTimeout=5 \
    -i "$WORKDIR/simpletest-key" ec2-user@$IP1 true 2>/dev/null && break
  sleep 5
done
scp -o StrictHostKeyChecking=accept-new -i "$WORKDIR/simpletest-key" \
  "$WORKDIR/counting_echo.py" "$WORKDIR/counting-echo.service" ec2-user@$IP1:/tmp/
ssh_inst 'sudo mv /tmp/counting_echo.py /opt/ && \
  sudo mv /tmp/counting-echo.service /etc/systemd/system/ && \
  sudo systemctl daemon-reload && \
  sudo systemctl enable --now counting-echo'
ssh_inst 'systemctl is-active counting-echo'   # expect: active
```

**CHECK**: `is-active` prints `active`.

## Phase 5 — confirm it runs (direct, by IP)

```sh
curl -s http://$IP1:8000/hello   # expect: count=1 path=/hello ...
curl -s http://$IP1:8000/again   # expect: count=2 path=/again ...
```

**CHECK**: two responses, counter incremented. (Note the `http://` — ground
rules; there is no https endpoint and there never will be in this exercise.)

## Phase 6 — confirm it survives restart

```sh
ssh_inst 'sudo reboot' || true   # connection drop is expected
# bounded poll until it answers again (typically < 90s, cap 5 min):
for i in $(seq 1 60); do
  curl -s --max-time 3 http://$IP1:8000/postreboot && break
  sleep 5
done
```

**CHECK**: response arrives WITHOUT any manual start, proving
`systemctl enable` took; counter is back to `count=1` — the reset itself is
the proof that this is a fresh process after a real reboot. Public IP does
NOT change on reboot (only on stop/start) — verify `$IP1` still answers.
If the loop exhausts 60 attempts, the phase FAILS.

## Phase 7 — DNS: `simpletest.example.com` → instance IP

TTL 60 so later phases don't wait on caches.

```sh
CHANGE_ID=$(aws route53 change-resource-record-sets --hosted-zone-id Z123456789EXAMPLE \
  --change-batch "{\"Changes\":[{\"Action\":\"UPSERT\",\"ResourceRecordSet\":{
    \"Name\":\"simpletest.example.com.\",\"Type\":\"A\",\"TTL\":60,
    \"ResourceRecords\":[{\"Value\":\"$IP1\"}]}}]}" \
  --query 'ChangeInfo.Id' --output text)
r53_wait "$CHANGE_ID"                                 # bounded INSYNC wait (ground rules)
# two-probe DNS assertion:
dig +short simpletest.example.com @ns-123.awsdns-45.net   # expect: $IP1
dig +short simpletest.example.com @8.8.8.8                # expect: $IP1
# canonical HTTP check — resolver-independent:
curl -s --resolve simpletest.example.com:8000:$IP1 \
  http://simpletest.example.com:8000/via-dns   # expect: count=N ... host=simpletest.example.com:8000
# bonus (non-gating): bare-hostname curl; may fail under split DNS/VPN:
curl -s http://simpletest.example.com:8000/via-dns-bare || echo "(local resolver quirk — not a failure)"
```

**CHECK**: both digs return `$IP1`; the `--resolve` curl succeeds. This is
the fragile pattern: a DNS A record hard-pointing at an ephemeral IP.

## Phase 8 — revoke SSH (deployment is done)

Everything after this phase must work without SSH. Revoke **by lookup**, not
by reconstructing the CIDR from `$MYIP` — this works even if the operator's
network changed mid-run, and it removes any port-22 rule that exists:

```sh
RULES=$(aws ec2 describe-security-groups --group-ids $SG_INST \
  --query 'SecurityGroups[0].IpPermissions[?FromPort==`22`]' --output json)
aws ec2 revoke-security-group-ingress --group-id $SG_INST --ip-permissions "$RULES"
aws ec2 describe-security-groups --group-ids $SG_INST \
  --query 'SecurityGroups[0].IpPermissions'   # expect: ONLY port 8000 remains
ssh -o ConnectTimeout=5 -i "$WORKDIR/simpletest-key" ec2-user@$IP1 true; echo "exit=$?"
```

**CHECK**: rule list shows only 8000/tcp. The ssh probe **must not
succeed**: exit 0 fails this phase; any nonzero exit passes (ssh typically
reports **255** on connection failure — the assertion is unreachability, not
a specific code). The service still answers on 8000.

## Phase 9 — the ALB

Cheapest ALB possible: one HTTP :80 listener, one target group, our single
instance as target. Also open 8000 from the ALB's SG into the instance SG
(the existing 0.0.0.0/0 rule already covers it, but the SG-to-SG rule is the
correct pattern and survives a later tightening of the world rule).

```sh
aws ec2 authorize-security-group-ingress --group-id $SG_INST \
  --protocol tcp --port 8000 --source-group $SG_ALB

TG=$(aws elbv2 create-target-group --name simpletest-tg --protocol HTTP \
  --port 8000 --vpc-id $VPC --target-type instance \
  --health-check-path / --health-check-interval-seconds 10 \
  --healthy-threshold-count 2 --unhealthy-threshold-count 2 \
  --tags Key=Project,Value=simpletest \
  --query 'TargetGroups[0].TargetGroupArn' --output text)
aws elbv2 register-targets --target-group-arn $TG --targets Id=$IID

ALB=$(aws elbv2 create-load-balancer --name simpletest-alb --type application \
  --scheme internet-facing --subnets $SUBNET_A $SUBNET_B \
  --security-groups $SG_ALB --tags Key=Project,Value=simpletest \
  --query 'LoadBalancers[0].LoadBalancerArn' --output text)
aws elbv2 create-listener --load-balancer-arn $ALB --protocol HTTP --port 80 \
  --default-actions Type=forward,TargetGroupArn=$TG
aws elbv2 wait load-balancer-available --load-balancer-arns $ALB
ALB_DNS=$(aws elbv2 describe-load-balancers --load-balancer-arns $ALB \
  --query 'LoadBalancers[0].DNSName' --output text)
ALB_ZONE=$(aws elbv2 describe-load-balancers --load-balancer-arns $ALB \
  --query 'LoadBalancers[0].CanonicalHostedZoneId' --output text)
# bounded poll until target healthy (health interval 10s; cap 5 min):
for i in $(seq 1 30); do
  STATE=$(aws elbv2 describe-target-health --target-group-arn $TG \
    --query 'TargetHealthDescriptions[0].TargetHealth.State' --output text)
  [ "$STATE" = healthy ] && break
  sleep 10
done; echo "target: $STATE"
```

**CHECK**: ALB state `active`; target health `healthy` (FAIL if the loop
exhausts); `curl -s http://$ALB_DNS/via-alb-raw` returns a `count=...`
response. Record `$TG`, `$ALB`, `$ALB_DNS`, `$ALB_ZONE` in the scratch file.

## Phase 10 — DNS: `simplealb.example.com` → ALB (alias)

Alias, not A-to-IP: the ALB's own IPs rotate, so an alias record is the only
correct way to point at it.

```sh
CHANGE_ID=$(aws route53 change-resource-record-sets --hosted-zone-id Z123456789EXAMPLE \
  --change-batch "{\"Changes\":[{\"Action\":\"UPSERT\",\"ResourceRecordSet\":{
    \"Name\":\"simplealb.example.com.\",\"Type\":\"A\",
    \"AliasTarget\":{\"HostedZoneId\":\"$ALB_ZONE\",
      \"DNSName\":\"$ALB_DNS\",\"EvaluateTargetHealth\":false}}}]}" \
  --query 'ChangeInfo.Id' --output text)
r53_wait "$CHANGE_ID"
# two-probe DNS assertion — the two answers are SUBSETS of the ALB's IP set
# and will usually differ; do NOT compare them (ground rules):
ALB_IP=$(dig +short simplealb.example.com @ns-123.awsdns-45.net | head -1)
dig +short simplealb.example.com @8.8.8.8   # expect: >=1 A record; ANY member of the set
# canonical HTTP check — resolver-independent, pinned to an authoritative IP:
curl -s --resolve simplealb.example.com:80:$ALB_IP \
  http://simplealb.example.com/via-alb      # expect: count=N ... host=simplealb.example.com
```

**CHECK**: authoritative dig returns at least one IP; `dig @8.8.8.8` returns
at least one IP (any member of the ALB's set — byte-for-byte match with
authoritative is NOT required and will often not hold); the `--resolve` curl
succeeds (http://, port 80, no TLS). Log both dig outputs in the run log.

## Phase 11 — stop, wait, start: watch the IP change

```sh
aws ec2 stop-instances --instance-ids $IID
aws ec2 wait instance-stopped --instance-ids $IID
aws ec2 start-instances --instance-ids $IID
aws ec2 wait instance-running --instance-ids $IID
IP2=$(aws ec2 describe-instances --instance-ids $IID \
  --query 'Reservations[0].Instances[0].PublicIpAddress' --output text)
echo "old=$IP1 new=$IP2"
```

**CHECK**: `$IP2 != $IP1` (this is why no EIP was attached). Record `IP2` as
a NEW scratch variable — **do not overwrite `IP1`**, teardown needs it.
Private IP is unchanged — which is why the ALB's instance-id target needs no
re-registration.

## Phase 12 — the punchline

```sh
# (a) the A-record path is now correctly BROKEN.
#     Assertion 1 — DNS is stale (authoritative still answers the old IP):
dig +short simpletest.example.com @ns-123.awsdns-45.net   # expect: $IP1, NOT $IP2
#     Assertion 2 — that stale IP is dead (curl pinned to $IP1, no resolver involved):
curl -s --max-time 10 --resolve simpletest.example.com:8000:$IP1 \
  http://simpletest.example.com:8000/ ; echo "exit=$?"

# (b) the ALB path heals itself — re-run the Phase 9 bounded health poll
#     until healthy, then:
ALB_IP=$(dig +short simplealb.example.com @ns-123.awsdns-45.net | head -1)
curl -s --resolve simplealb.example.com:80:$ALB_IP \
  http://simplealb.example.com/after-restart   # expect: count=... (fresh counter)
```

**CHECK (a)** — BOTH must hold:
- authoritative dig returns `$IP1` (the stale value);
- the pinned curl fails with a *connectivity* error and no `count=` body.
  Acceptable exit codes: **28** (timed out — ideal) or **7** (refused — the
  IP was reassigned to a host with nothing on 8000). A `200` without
  `count=` also passes — the IP went to a stranger's server; same lesson,
  louder. Exit **6** (could not resolve) must NOT occur here — `--resolve`
  bypasses the resolver; if you see it, the command is wrong, not the DNS.

**CHECK (b)**: target healthy again; response is HTTP 200 with `count=` and
`host=simplealb.example.com`. This is the entire moral of the exercise.

## Phase 13 — teardown (exact order matters)

DNS first (never leave names pointing at resources about to die), then ALB
before target group (TG can't be deleted while a listener forwards to it),
then instance, then SGs (only deletable once nothing references them), then
the key pair.

```sh
# 1. both DNS records (DELETE requires exact current values):
#    simpletest.example.com  A TTL60 -> $IP1  (yes, $IP1 — the frozen stale value)
#    simplealb.example.com   A ALIAS -> $ALB_DNS / $ALB_ZONE
#    DELETE batches return PENDING ChangeInfo just like UPSERTs — capture
#    each change ID and r53_wait it, so the names are provably gone before
#    the compute they point at is:
#      CHANGE_ID=$(aws route53 change-resource-record-sets ... --query 'ChangeInfo.Id' --output text)
#      r53_wait "$CHANGE_ID"
# 2. ALB (listener dies with it):
aws elbv2 delete-load-balancer --load-balancer-arn $ALB
aws elbv2 wait load-balancers-deleted --load-balancer-arns $ALB
# 3. target group — listener detachment can lag the wait above, so bounded
#    retry on ResourceInUse (fail loudly if still stuck after ~1 min):
for i in $(seq 1 12); do
  aws elbv2 delete-target-group --target-group-arn $TG 2>/dev/null && break
  sleep 5
done
aws elbv2 describe-target-groups --target-group-arns $TG 2>&1 | grep -q TargetGroupNotFound \
  && echo "TG deleted" || echo "TG STILL EXISTS — investigate before continuing"
# 4. instance:
aws ec2 terminate-instances --instance-ids $IID
aws ec2 wait instance-terminated --instance-ids $IID
# 5. security groups — instance SG references ALB SG, so delete SG_INST first:
aws ec2 delete-security-group --group-id $SG_INST
aws ec2 delete-security-group --group-id $SG_ALB
# 6. key pair and the whole temp workdir (key material + scratch file):
aws ec2 delete-key-pair --key-name simpletest-key
rm -rf "$WORKDIR"
```

## Phase 14 — confirm it is ALL torn down

Every command below must come back empty, with ONE documented exception:
terminated instances remain **visible** (state `terminated`) for up to ~1h
after Phase 13, and **repeated runs accumulate multiple `terminated` rows** —
all of them are a pass. The pass criterion for instances is "nothing in any
state other than `terminated`" — do not read lingering `terminated` rows as
a failure, and do not read this exception as covering anything else. (To
silence the noise entirely, append
`--filters Name=instance-state-name,Values=pending,running,shutting-down,stopping,stopped`
to the instances query and require literally empty output.)

```sh
aws ec2 describe-instances --filters Name=tag:Project,Values=simpletest \
  --query 'Reservations[].Instances[].[InstanceId,State.Name]'   # [] or terminated only
aws elbv2 describe-load-balancers \
  --query 'LoadBalancers[?contains(LoadBalancerName,`simpletest`)]'   # []
aws elbv2 describe-target-groups \
  --query 'TargetGroups[?contains(TargetGroupName,`simpletest`)]'     # []
aws ec2 describe-security-groups --filters Name=tag:Project,Values=simpletest \
  --query 'SecurityGroups[].GroupId'                                  # []
aws ec2 describe-key-pairs --key-names simpletest-key 2>&1 | grep -c NotFound  # 1 (NotFound = pass)
aws ec2 describe-volumes --filters Name=tag:Project,Values=simpletest \
  --query 'Volumes[].VolumeId'                                        # []
aws ec2 describe-addresses --query 'Addresses[?Tags[?Value==`simpletest`]]' # [] (none were ever created)
aws route53 list-resource-record-sets --hosted-zone-id Z123456789EXAMPLE \
  --query 'ResourceRecordSets[?contains(Name,`simple`)]'              # []
```

Optionally finish with your inventory tooling of choice and confirm the
account is back to its pre-exercise baseline: no `simpletest-*` resource of
any kind, no load balancers, no stray EIPs, no `simple*` DNS records.

**CHECK**: all empty / back to the pre-exercise baseline. The exercise is
complete when this phase passes, not before.

## Appendix A — emergency teardown (failed or partial run)

Do not try to resume a broken run; tear down and rerun from Phase 0 (a full
run costs pennies). This sequence is Phase 13 made tolerant: every step
no-ops cleanly when its resource never got created. Discover what exists by
tag, then delete in the same order.

```sh
export AWS_DEFAULT_REGION=us-west-1
# discover (also re-derives IDs if the scratch file is lost):
aws ec2 describe-instances --filters Name=tag:Project,Values=simpletest \
  --query 'Reservations[].Instances[].[InstanceId,State.Name]' --output text
aws elbv2 describe-load-balancers --query 'LoadBalancers[?contains(LoadBalancerName,`simpletest`)].LoadBalancerArn' --output text
aws elbv2 describe-target-groups --query 'TargetGroups[?contains(TargetGroupName,`simpletest`)].TargetGroupArn' --output text
aws ec2 describe-security-groups --filters Name=tag:Project,Values=simpletest \
  --query 'SecurityGroups[].GroupId' --output text
aws route53 list-resource-record-sets --hosted-zone-id Z123456789EXAMPLE \
  --query 'ResourceRecordSets[?contains(Name,`simple`)]'

# then, in order, skipping whatever is absent (|| true throughout):
# 1. delete any simple* DNS records (fetch each record's exact current value
#    from the query above and DELETE it verbatim)
# 2. delete-load-balancer + wait                          || true
# 3. delete-target-group (bounded retry loop from Phase 13) || true
# 4. terminate-instances + wait                           || true
# 5. delete-security-group SG_INST, then SG_ALB           || true
#    (if SG_INST delete fails with DependencyViolation, wait for the
#     instance to finish terminating and retry)
# 6. delete-key-pair simpletest-key                       || true
# 7. rm -rf "$WORKDIR"                                    || true
```

Finish with the Phase 14 checklist — emergency or not, the run is not over
until Phase 14 passes.
