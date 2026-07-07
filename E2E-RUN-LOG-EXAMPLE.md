# E2E Run Log — example (sanitized)

A real, complete run of `E2E-HARNESS.md` rev 3 — every phase passed on the first attempt with no improvisation. Account ID, domain, IPs, and resource IDs are masked with documentation placeholders (`111122223333`, `example.com`, RFC 5737 addresses); the outputs are otherwise verbatim.

Account `111122223333`, region `us-west-1`.

## Phase 0 — preflight

**CHECK: PASSED**

```
aws sts get-caller-identity → Account 111122223333
VPC=vpc-0123456789abcdef0
SUBNET_A=subnet-0aaaa1111bbbb2222 (us-west-1b)
SUBNET_B=subnet-0cccc3333dddd4444 (us-west-1c)
MYIP=198.51.100.75
WORKDIR=/tmp/tmp.XXXXXXXXXX
```

Leftover check: only terminated instances from prior runs (acceptable); no SGs; key clean; scratch clean.

## Phase 1 — the application

**CHECK: PASSED**

```
curl localhost:8000/x → count=1 path=/x host=localhost:8000
curl localhost:8000/x → count=2 path=/x host=localhost:8000
```

## Phase 2 — security groups

**CHECK: PASSED**

```
SG_INST=sg-0aaaabbbbcccc1111  (22/tcp from 198.51.100.75/32, 8000/tcp from 0.0.0.0/0)
SG_ALB=sg-0ddddeeeeffff2222   (80/tcp from 0.0.0.0/0)
```

## Phase 3 — key pair and instance

**CHECK: PASSED**

```
AMI=ami-083df663551d24a71
IID=i-0123456789abcdef0
IP1=203.0.113.10 (ephemeral, no EIP)
PRIVATE_IP=172.31.11.21
State: running
```

## Phase 4 — deploy over SSH

**CHECK: PASSED**

```
systemctl is-active counting-echo → active
```

## Phase 5 — confirm it runs (direct, by IP)

**CHECK: PASSED**

```
curl http://203.0.113.10:8000/hello → count=1 path=/hello ...
curl http://203.0.113.10:8000/again → count=2 path=/again ...
```

## Phase 6 — confirm it survives restart

**CHECK: PASSED**

```
post-reboot curl → count=1 path=/postreboot (counter reset proves fresh process)
IP1 unchanged: 203.0.113.10
```

## Phase 7 — DNS: simpletest.example.com → instance IP

**CHECK: PASSED**

```
dig @ns-123.awsdns-45.net → 203.0.113.10
dig @8.8.8.8 → 203.0.113.10
curl --resolve simpletest.example.com:8000:203.0.113.10 → count=2 path=/via-dns host=simpletest.example.com:8000
```

## Phase 8 — revoke SSH

**CHECK: PASSED**

```
IpPermissions: only 8000/tcp from 0.0.0.0/0
ssh probe exit=255 (unreachable, as required)
service still answers on 8000
```

## Phase 9 — the ALB

**CHECK: PASSED**

```
TG=arn:aws:elasticloadbalancing:us-west-1:111122223333:targetgroup/simpletest-tg/0123456789abcdef
ALB=arn:aws:elasticloadbalancing:us-west-1:111122223333:loadbalancer/app/simpletest-alb/abcdef0123456789
ALB_DNS=simpletest-alb-1234567890.us-west-1.elb.amazonaws.com
ALB_ZONE=Z368ELLRRE2KJ0
target: healthy
curl http://ALB_DNS/via-alb-raw → count=13 ...
```

## Phase 10 — DNS: simplealb.example.com → ALB (alias)

**CHECK: PASSED**

```
dig @ns-123.awsdns-45.net → 198.51.100.11
dig @8.8.8.8 → 198.51.100.11, 198.51.100.12
curl --resolve simplealb.example.com:80:198.51.100.11 → count=20 path=/via-alb host=simplealb.example.com
```

(Note the two probes returning different subsets of the ALB's IP set — expected per the DNS ground rules; not compared.)

## Phase 11 — stop, wait, start: watch the IP change

**CHECK: PASSED**

```
old=203.0.113.10 new=203.0.113.20 (IP changed as expected)
PRIVATE_IP unchanged: 172.31.11.21
```

## Phase 12 — the punchline

**CHECK: PASSED**

**(a) stale A-record path broken:**
```
dig @ns-123.awsdns-45.net → 203.0.113.10 (stale IP1, not new IP2)
curl --resolve simpletest.example.com:8000:203.0.113.10 → exit=28 (timeout)
```

**(b) ALB path heals:**
```
target: healthy
curl --resolve simplealb.example.com:80:198.51.100.11 → count=8 path=/after-restart host=simplealb.example.com
```

## Phase 13 — teardown

**CHECK: PASSED**

DNS records deleted (simpletest.example.com using frozen IP1=203.0.113.10, simplealb.example.com alias). ALB, target group, instance, security groups, and key pair all deleted. WORKDIR removed.

## Phase 14 — confirm it is ALL torn down

**CHECK: PASSED**

```
instances: i-0123456789abcdef0 terminated (+ terminated rows from prior runs)
load balancers: []
target groups: []
security groups: []
key pair: NotFound
volumes: []
addresses: []
route53 simple*: []
account sweep: only pre-existing baseline resources remain
```

**Exercise complete.**
