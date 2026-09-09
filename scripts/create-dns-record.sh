#!/usr/bin/env bash
set -euo pipefail

# Opretter A-record for loft.test.gihc.online hos Simply.com, så Loft kan nås
# på k3s-clusteret. Idempotent — springer over hvis recorden findes.
#
# Credentials hentes fra `pass` (samme mønster som capture):
#   pass insert simply/account
#   pass insert simply/api-key

DOMAIN="gihc.online"
RECORD_NAME="loft.test"
VPS_IP="65.109.233.92"

SIMPLY_ACCOUNT="$(pass simply/account)"
SIMPLY_API_KEY="$(pass simply/api-key)"

API_URL="https://api.simply.com/2/my/products/${DOMAIN}/dns/records/"

echo "==> Henter DNS records for ${DOMAIN}..."
RECORDS=$(curl -sf -u "${SIMPLY_ACCOUNT}:${SIMPLY_API_KEY}" "${API_URL}")

EXISTS=$(RECORD_NAME="$RECORD_NAME" python3 -c '
import json, os, sys
records = json.load(sys.stdin).get("records", [])
name = os.environ["RECORD_NAME"]
print("yes" if any(r.get("type") == "A" and r.get("name") == name for r in records) else "no")
' <<<"$RECORDS")

if [ "$EXISTS" = "yes" ]; then
    echo "==> A-record for ${RECORD_NAME}.${DOMAIN} findes allerede, springer over."
    exit 0
fi

echo "==> Opretter A-record ${RECORD_NAME}.${DOMAIN} -> ${VPS_IP}..."
curl -sf -u "${SIMPLY_ACCOUNT}:${SIMPLY_API_KEY}" \
    -X POST "${API_URL}" \
    -H "Content-Type: application/json" \
    -d "{\"type\":\"A\",\"name\":\"${RECORD_NAME}\",\"data\":\"${VPS_IP}\",\"ttl\":3600}" \
    -o /dev/null

echo "==> Oprettet."
