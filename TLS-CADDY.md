# TLS with Caddy: single names for free, wildcards through Route 53

A runbook for putting a real, auto-renewing Let's Encrypt certificate in front of a small service on one Linux box, using Caddy as the TLS terminator. Two paths, from cheapest to most capable, plus the glue that ties the wildcard path to this repository's `awsw` tool so the server never holds broad AWS credentials.

Everything here is placeholder-only. Substitute before running:

| Placeholder | Meaning |
|---|---|
| `example.com` | a zone you control, hosted in Route 53 |
| `console.example.com` | one hostname on that zone that should resolve to the box |
| `Z123456789EXAMPLE` | the zone's hosted-zone ID (`./awsw r53` prints it) |
| `ns-123.awsdns-45.net` | one of the zone's authoritative name servers (`./awsw r53` prints the NS record) |
| `APP_PORT` | the loopback port your application listens on |

**Executor contract**: run sections in order; each ends with a CHECK. Nothing here modifies AWS except the TXT records in section 3, and those are created and deleted inside one certbot run.

## 0. Why Caddy

- It terminates TLS, redirects 80 to 443, and reverse-proxies to a loopback port with a four-line config.
- For ordinary hostnames it obtains and renews certificates by itself over HTTP-01 or TLS-ALPN-01. No certbot, no cron, no hooks.
- For a wildcard it cannot do HTTP-01, because the challenge must be a DNS TXT record. Then certbot issues and Caddy only serves (section 3).
- The distro package drops privileges to a `caddy` user, binds 80 and 443 through `CAP_NET_BIND_SERVICE`, and runs with `ProtectSystem=full` and `PrivateTmp`. Keep that unit; do not run it as root.

The nginx-plus-certbot-webroot pattern this document replaces works, but it needs a writable challenge directory, an nginx reload hook, and a certbot config directory per name. Caddy removes all three for single names.

## 1. Install and expose

```sh
sudo apt-get install -y caddy          # Debian/Ubuntu package; pulls in the systemd unit
systemctl is-enabled caddy             # expect: enabled
sudo ss -ltnp | grep -E ':80 |:443 '   # expect: caddy on both
```

Open 80 and 443 in whatever firewall or security group fronts the box. Port 80 must stay open: Let's Encrypt validates over it and Caddy redirects on it.

The application must listen on loopback only (`127.0.0.1:APP_PORT`), never on `0.0.0.0`. Caddy is the only thing that faces the network.

**CHECK**: `curl -sI http://127.0.0.1` returns a Caddy response; the application answers on `curl -s http://127.0.0.1:APP_PORT/`.

## 2. Path A: one hostname, zero ceremony

### 2.1 DNS

Create the A record with `awsw`'s read inventory to confirm first, then any writer you trust for A records. `awsw` deliberately writes only TXT records, so the A record is created with the console or another tool:

```sh
./awsw r53                              # confirm the zone and that the name does not exist yet
# create: console.example.com. A 300 -> the box's public IPv4
dig +short console.example.com @ns-123.awsdns-45.net   # expect the box's IP
dig +short console.example.com @8.8.8.8                # expect the same
```

Two probes, authoritative and public, as in the E2E harness: the local resolver is not trusted.

### 2.2 Caddyfile

```caddyfile
console.example.com {
    reverse_proxy 127.0.0.1:APP_PORT
}
```

```sh
sudo install -m 644 Caddyfile /etc/caddy/Caddyfile
sudo caddy validate --config /etc/caddy/Caddyfile
sudo systemctl reload caddy
```

That is the whole setup. Caddy sees a site with a public hostname, asks Let's Encrypt for a certificate over HTTP-01, stores it under `/var/lib/caddy/.local/share/caddy/certificates/`, and renews it at two thirds of its lifetime. Nothing else to schedule.

**CHECK**: `curl -sS https://console.example.com/` succeeds with full certificate verification. `sudo journalctl -u caddy --since -10min | grep -i 'certificate obtained'` shows the issuance. `curl -sI http://console.example.com/` returns a 308 to https.

### 2.3 Ephemeral boxes without a zone

If the box has a public IP but no domain yet, a public wildcard DNS service such as `sslip.io` maps `A-B-C-D.sslip.io` to the IP `A.B.C.D` with no setup. Put that name in the Caddyfile and Path A works unchanged. Good for throwaway VMs; never for anything with a lifetime, because anyone who later gets that IP gets that name.

## 3. Path B: a wildcard, certbot issues, Caddy serves

Use this when `*.example.com` must be covered, or when one certificate must serve many names on one box. HTTP-01 cannot prove a wildcard, so the challenge is a TXT record at `_acme-challenge.example.com`. certbot drives the challenge; a deploy hook hands the result to Caddy.

### 3.1 The challenge hooks, built on awsw

The usual choice is certbot's `dns-route53` plugin. It works, but it needs an AWS SDK on the server plus credentials that the plugin discovers through the standard chain, which encourages dropping a broad key into `/root/.aws/credentials`. The hooks below use `awsw r53 --upsert-txt` and `--delete-txt` instead: a single static binary, no SDK, and a credential that can only change TXT records in one zone.

Install `awsw` on the server (`cargo install --git <this repository> --locked awsw`, or copy a release build) and write the two hooks. They live in `/etc/letsencrypt/acme-hooks/`, mode 700, owned by root.

`/etc/letsencrypt/acme-hooks/auth.sh`:

```sh
#!/bin/sh
# certbot manual-auth-hook. Called once per name in the order; a wildcard plus
# its apex both validate at the same _acme-challenge name, so values are
# accumulated and the record is UPSERTed with the full set each time.
set -eu
ZONE_ID=Z123456789EXAMPLE
AUTH_NS=ns-123.awsdns-45.net
STATE=/run/acme-hooks; install -d -m 700 "$STATE"
NAME="_acme-challenge.${CERTBOT_DOMAIN#\*.}"
echo "$CERTBOT_VALIDATION" >> "$STATE/$NAME.values"
set --
while IFS= read -r v; do set -- "$@" --value "$v"; done < "$STATE/$NAME.values"
awsw r53 --upsert-txt "$NAME" --zone-id "$ZONE_ID" --ttl 60 "$@" >/dev/null
# bounded wait until the authoritative server returns the newest value:
i=0
until dig +short TXT "$NAME" @"$AUTH_NS" | grep -qF "$CERTBOT_VALIDATION"; do
  i=$((i+1)); [ "$i" -le 30 ] || { echo "TXT not visible after 60s" >&2; exit 1; }
  sleep 2
done
```

`/etc/letsencrypt/acme-hooks/cleanup.sh`:

```sh
#!/bin/sh
# certbot manual-cleanup-hook. Route 53 DELETE must match TTL and the complete
# value set, so delete once with every accumulated value, then forget them.
set -eu
ZONE_ID=Z123456789EXAMPLE
STATE=/run/acme-hooks
NAME="_acme-challenge.${CERTBOT_DOMAIN#\*.}"
[ -f "$STATE/$NAME.values" ] || exit 0
set --
while IFS= read -r v; do set -- "$@" --value "$v"; done < "$STATE/$NAME.values"
awsw r53 --delete-txt "$NAME" --zone-id "$ZONE_ID" --ttl 60 "$@" >/dev/null
rm -f "$STATE/$NAME.values"
```

`awsw` reads credentials from `AWS_ACCESS_KEY_ID` and `AWS_SECRET_ACCESS_KEY`, or from `~/.aws/credentials` of the invoking user. certbot runs as root from its timer, so either put the key pair in `/root/.aws/credentials` (mode 600) or export the two variables from a root-only file sourced at the top of both hooks. Nothing else on the box needs them.

### 3.2 The credential: one zone, TXT only

Create a dedicated IAM user for this box and attach only this policy. No group membership, no console password, one access key.

```json
{
  "Version": "2012-10-17",
  "Statement": [
    {
      "Effect": "Allow",
      "Action": "route53:ChangeResourceRecordSets",
      "Resource": "arn:aws:route53:::hostedzone/Z123456789EXAMPLE",
      "Condition": {
        "ForAllValues:StringEquals": {
          "route53:ChangeResourceRecordSetsRecordTypes": ["TXT"],
          "route53:ChangeResourceRecordSetsActions": ["UPSERT", "DELETE"]
        },
        "ForAllValues:StringLike": {
          "route53:ChangeResourceRecordSetsNormalizedRecordNames": ["_acme-challenge.*"]
        }
      }
    }
  ]
}
```

That is the complete surface: UPSERT or DELETE a TXT record whose name starts with `_acme-challenge.` in one zone. It cannot read the zone, cannot touch A records, cannot create zones. If a key with this policy leaks, the worst outcome is a bogus ACME challenge record.

The `dns-route53` plugin needs more than this (`ListHostedZones` and `GetChange` at minimum), which is one reason to prefer the hooks.

Name the user after the box and zone so the IAM audit (`./awsw iam`) explains itself, and record the key creation date; the audit flags keys older than 90 days.

### 3.3 Issue

Rehearse against the staging CA first. A failed staging run costs nothing; a failed production run counts against rate limits.

```sh
sudo certbot certonly --manual --preferred-challenges dns \
  --manual-auth-hook /etc/letsencrypt/acme-hooks/auth.sh \
  --manual-cleanup-hook /etc/letsencrypt/acme-hooks/cleanup.sh \
  --key-type ecdsa --agree-tos --no-eff-email -m ops@example.com \
  -d example.com -d '*.example.com' \
  --dry-run
# then the same command without --dry-run
```

**CHECK**: `sudo certbot certificates` lists `example.com` with `*.example.com` in the domains and about 90 days validity. `./awsw r53` shows no `_acme-challenge` record left in the zone: the cleanup hook ran.

### 3.4 Hand the certificate to Caddy

certbot writes to `/etc/letsencrypt/live/`, readable by root only. Caddy runs as `caddy`. The deploy hook copies the two files atomically into Caddy's own tree and reloads. certbot runs every deploy hook after a successful renewal, so this is the entire renewal story.

`/etc/letsencrypt/renewal-hooks/deploy/example-caddy`, mode 755:

```sh
#!/bin/sh
set -eu
[ "${RENEWED_LINEAGE:-}" = "/etc/letsencrypt/live/example.com" ] || exit 0
src=/etc/letsencrypt/live/example.com
dst=/var/lib/caddy/certs/example.com
install -d -m 700 -o caddy -g caddy "$dst"
install -m 600 -o caddy -g caddy "$src/fullchain.pem" "$dst/.fullchain.pem.new"
install -m 600 -o caddy -g caddy "$src/privkey.pem"   "$dst/.privkey.pem.new"
mv -f "$dst/.fullchain.pem.new" "$dst/fullchain.pem"
mv -f "$dst/.privkey.pem.new"   "$dst/privkey.pem"
systemctl reload caddy
```

Run it once by hand for the first install:

```sh
sudo RENEWED_LINEAGE=/etc/letsencrypt/live/example.com /etc/letsencrypt/renewal-hooks/deploy/example-caddy
```

Then point Caddy at the files:

```caddyfile
console.example.com {
    tls /var/lib/caddy/certs/example.com/fullchain.pem /var/lib/caddy/certs/example.com/privkey.pem
    reverse_proxy 127.0.0.1:APP_PORT
}

example.com, *.example.com {
    tls /var/lib/caddy/certs/example.com/fullchain.pem /var/lib/caddy/certs/example.com/privkey.pem
    abort
}
```

The second block matters: without it Caddy would try to obtain its own certificate for any unlisted name that arrives with SNI under the zone, fail HTTP-01 for the wildcard, and log noise forever. `abort` closes those connections instead.

**CHECK**: `sudo caddy validate --config /etc/caddy/Caddyfile`; `sudo systemctl reload caddy`; `curl -sS https://console.example.com/` verifies; `openssl s_client -connect console.example.com:443 -servername anything.example.com </dev/null 2>/dev/null | openssl x509 -noout -ext subjectAltName` shows the wildcard.

### 3.5 Renewal, verified rather than assumed

```sh
systemctl list-timers certbot.timer          # active, next run within 12h
sudo certbot renew --dry-run                 # exercises auth, cleanup, and deploy hooks against staging
sudo journalctl -u caddy --since -5min | grep -i reload   # the deploy hook reloaded Caddy
```

**CHECK**: dry run succeeds; the zone shows no leftover `_acme-challenge` record; Caddy logged a reload.

## 4. Choosing between the paths

| | Path A: Caddy alone | Path B: certbot + hooks + Caddy |
|---|---|---|
| Names | any list of concrete hostnames | wildcard, or many names on one cert |
| AWS access on the box | none | one TXT-only key for one zone |
| Moving parts | Caddyfile | Caddyfile, two hooks, deploy hook, timer |
| Renewal failure mode | Caddy logs, keeps serving the old cert until expiry | certbot timer logs; deploy hook never runs, Caddy keeps the old files |
| Port 80 required | yes | no, once issued |

Default to Path A. Reach for Path B only when a wildcard is actually required.

## 5. Undo

Path A: remove the site block, reload Caddy, delete the A record. Caddy keeps the old certificate on disk harmlessly; delete its directory under `/var/lib/caddy/.local/share/caddy/certificates/` if you want it gone.

Path B: `sudo certbot delete --cert-name example.com`, remove the deploy hook and the Caddy cert directory, reload Caddy, then deactivate and delete the IAM user's access key. Confirm with `./awsw iam` that the user has no active keys, and with `./awsw r53` that no `_acme-challenge` record remains.
