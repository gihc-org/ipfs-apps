#!/usr/bin/env bash
# Smoke test for et deployet Loft-miljø.
#
# Dækker gæste-loft-flowet ende-til-ende mod ét origin (frontend + API + WS):
# /healthz, opret loft, hent metadata, WebSocket-join med to gæster,
# signal/media-state-relay, luk loft med ejer-nøgle og (valgfrit) at rækken
# faktisk er væk i databasen.
#
# Brug:
#   ./scripts/smoke-test.sh https://loft.test.gihc.online
#   ./scripts/smoke-test.sh https://loft.test.gihc.online --namespace loft-test
#   ./scripts/smoke-test.sh https://loft.test.gihc.online --insecure   # staging-cert
#
# --namespace slår databasen op via `kubectl exec` i postgres-poden; uden
# flaget springes DB-verifikationen over (fx fra en maskine uden kubeconfig).

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

ORIGIN="${1:?Usage: $0 <origin> [--namespace <ns>] [--insecure]}"
shift

NAMESPACE=""
INSECURE=0
while [ $# -gt 0 ]; do
    case "$1" in
        --namespace)
            NAMESPACE="${2:?--namespace kræver et navn}"
            shift 2
            ;;
        --insecure)
            INSECURE=1
            shift
            ;;
        *)
            echo "Ukendt argument: $1" >&2
            exit 2
            ;;
    esac
done

ORIGIN="${ORIGIN%/}"
CURL=(curl -sS --max-time 10)
if [ "$INSECURE" = 1 ]; then
    CURL+=(-k)
fi

fail() {
    echo "FAIL: $1" >&2
    exit 1
}

ok() {
    echo "ok  $1"
}

# check <label> <forventet-status> <faktisk-status>
check() {
    local label="$1" expected="$2" actual="$3"
    [ "$actual" = "$expected" ] || fail "${label} — forventede ${expected}, fik ${actual}"
    ok "$label"
}

# request <metode> <sti> [ekstra curl-args...] → sætter STATUS og BODY
request() {
    local method="$1" path="$2"
    shift 2
    local response
    response=$("${CURL[@]}" -w $'\n%{http_code}' -X "$method" "${ORIGIN}${path}" "$@")
    STATUS="${response##*$'\n'}"
    BODY="${response%$'\n'*}"
}

json_get() {
    python3 -c 'import json,sys; print(json.load(sys.stdin).get(sys.argv[1], ""))' "$1"
}

LOFT_ID=""
OWNER_TOKEN=""

cleanup() {
    if [ -n "$LOFT_ID" ] && [ -n "$OWNER_TOKEN" ]; then
        "${CURL[@]}" -o /dev/null -X DELETE \
            -H "X-Owner-Token: ${OWNER_TOKEN}" \
            "${ORIGIN}/v1/lofts/${LOFT_ID}" || true
    fi
}
trap cleanup EXIT

# ── 1. Liveness ────────────────────────────────────────────────────────────
request GET /healthz
check "GET /healthz" "200" "$STATUS"
[ "$BODY" = "ok" ] || fail "GET /healthz — body var '${BODY}', forventede 'ok'"

# ── 2. Opret loft ──────────────────────────────────────────────────────────
LOFT_NAME="Smoke $RANDOM"
request POST /v1/lofts -H "Content-Type: application/json" -d "{\"name\":\"${LOFT_NAME}\"}"
check "POST /v1/lofts" "201" "$STATUS"

LOFT_ID=$(printf '%s' "$BODY" | json_get id)
OWNER_TOKEN=$(printf '%s' "$BODY" | json_get owner_token)
[ -n "$LOFT_ID" ] || fail "POST /v1/lofts — svar uden id: ${BODY}"
[ -n "$OWNER_TOKEN" ] || fail "POST /v1/lofts — svar uden owner_token: ${BODY}"
printf '%s' "$BODY" | grep -q "/loft.html?id=${LOFT_ID}" \
    || fail "POST /v1/lofts — url peger ikke på loftet: ${BODY}"
ok "POST /v1/lofts returnerer id, url og owner_token"

# ── 3. Metadata (link-åbning) ──────────────────────────────────────────────
request GET "/v1/lofts/${LOFT_ID}"
check "GET /v1/lofts/:id" "200" "$STATUS"
[ "$(printf '%s' "$BODY" | json_get name)" = "$LOFT_NAME" ] \
    || fail "GET /v1/lofts/:id — navn matcher ikke: ${BODY}"
if printf '%s' "$BODY" | grep -q owner_token; then
    fail "GET /v1/lofts/:id lækker owner_token: ${BODY}"
fi
ok "GET /v1/lofts/:id lækker ikke owner_token"

request GET "/v1/lofts/$(python3 -c 'import uuid; print(uuid.uuid4())')"
check "GET /v1/lofts/:ukendt-id (forventer 404)" "404" "$STATUS"

# ── 4. WebSocket: gæste-flow med to deltagere ──────────────────────────────
WS_ORIGIN="${ORIGIN/http:/ws:}"
WS_ORIGIN="${WS_ORIGIN/https:/wss:}"
WS_ARGS=("$WS_ORIGIN" "$LOFT_ID")
if [ "$INSECURE" = 1 ]; then
    WS_ARGS+=(--insecure)
fi
python3 "${SCRIPT_DIR}/smoke-ws.py" "${WS_ARGS[@]}" \
    || fail "WebSocket-gæsteflow mod ${WS_ORIGIN}/v1/ws/${LOFT_ID}"

# ── 5. Luk loft (ejer-nøgle) ───────────────────────────────────────────────
request DELETE "/v1/lofts/${LOFT_ID}"
check "DELETE /v1/lofts/:id uden ejer-nøgle (forventer 403)" "403" "$STATUS"

request DELETE "/v1/lofts/${LOFT_ID}" -H "X-Owner-Token: $(python3 -c 'import uuid; print(uuid.uuid4())')"
check "DELETE /v1/lofts/:id med forkert nøgle (forventer 403)" "403" "$STATUS"

request DELETE "/v1/lofts/${LOFT_ID}" -H "X-Owner-Token: ${OWNER_TOKEN}"
check "DELETE /v1/lofts/:id med ejer-nøgle" "204" "$STATUS"

request GET "/v1/lofts/${LOFT_ID}"
check "GET /v1/lofts/:id efter lukning (forventer 404)" "404" "$STATUS"

# Lukket loft må ikke kunne lukkes igen med samme nøgle
request DELETE "/v1/lofts/${LOFT_ID}" -H "X-Owner-Token: ${OWNER_TOKEN}"
check "DELETE /v1/lofts/:id igen (forventer 404)" "404" "$STATUS"

CLOSED_LOFT_ID="$LOFT_ID"
LOFT_ID=""
OWNER_TOKEN=""

# ── 6. Databasen (valgfrit, kræver kubectl-adgang) ─────────────────────────
if [ -n "$NAMESPACE" ]; then
    command -v kubectl >/dev/null || fail "--namespace kræver kubectl i PATH"
    COUNT=$(kubectl -n "$NAMESPACE" exec deploy/loft-postgres -- \
        psql -U loft -d loftdb -tAc "SELECT count(*) FROM lofts WHERE id = '${CLOSED_LOFT_ID}'" \
        | tr -d '[:space:]')
    [ "$COUNT" = "0" ] || fail "loftet findes stadig i databasen efter lukning (count=${COUNT})"
    ok "loftet er slettet i databasen (kubectl exec → psql)"
fi

echo ""
echo "All Loft smoke tests passed."
