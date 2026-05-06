#!/usr/bin/env bash
# Smoke test for the deployed chat API.
#
# Creates a temporary user directly in the database (bypassing CAPTCHA),
# exercises the key authenticated endpoints, then deletes the user.
#
# Usage:
#   ./scripts/smoke-test.sh <api_url> <postgres_user> <postgres_password> <postgres_db> <compose_dir>
#
# Example (local):
#   ./scripts/smoke-test.sh https://api.gihc.online chatuser secret chatdb /opt/chat
#
# In CI / Ansible the arguments are injected from vault variables.

set -euo pipefail

API="${1:?Usage: $0 <api_url> <postgres_user> <postgres_password> <postgres_db> <compose_dir>}"
PG_USER="${2:?}"
PG_PASS="${3:?}"
PG_DB="${4:?}"
COMPOSE_DIR="${5:?}"

TEST_USER="smoke_test_ci"
TEST_PASS="SmokeTest99!"
# argon2id hash of TEST_PASS (Argon2::default() — m=19456,t=2,p=1)
TEST_HASH='$argon2id$v=19$m=19456,t=2,p=1$JD46E95F0HZ6fa5RAezmjQ$ucT1cnSgRmpdOgc7YEGC9MhNhv7kJvDd/4jtSm3/fgQ'

CONTAINER=$(docker compose -f "${COMPOSE_DIR}/docker-compose.yml" ps -q postgres 2>/dev/null | head -1)
if [[ -z "$CONTAINER" ]]; then
  echo "ERROR: could not find postgres container in ${COMPOSE_DIR}" >&2
  exit 1
fi

psql_exec() {
  PGPASSWORD="$PG_PASS" docker exec -i "$CONTAINER" \
    psql -U "$PG_USER" "$PG_DB" -t -A -c "$1"
}

cleanup() {
  psql_exec "DELETE FROM users WHERE username = '${TEST_USER}';" > /dev/null 2>&1 || true
}
trap cleanup EXIT

# Remove any leftover test user from a previous failed run
cleanup

# Insert test user with known password hash (email_verified = TRUE so login works)
psql_exec "
  INSERT INTO users (username, password_hash, email, email_verified)
  VALUES ('${TEST_USER}', '${TEST_HASH}', '${TEST_USER}@smoke.test', TRUE);
" > /dev/null

fail() {
  echo "FAIL: $1" >&2
  exit 1
}

check() {
  local label="$1" expected="$2" actual="$3"
  if [[ "$actual" != "$expected" ]]; then
    fail "${label} — expected ${expected}, got ${actual}"
  fi
  echo "ok  ${label}"
}

# 1. Login
RESP=$(curl -sf -w "\n%{http_code}" -X POST "${API}/auth/token" \
  -H "Content-Type: application/json" \
  -d "{\"username\":\"${TEST_USER}\",\"password\":\"${TEST_PASS}\"}")
STATUS=$(echo "$RESP" | tail -1)
BODY=$(echo "$RESP" | head -1)
check "POST /auth/token" "200" "$STATUS"

TOKEN=$(echo "$BODY" | python3 -c "import sys,json; print(json.load(sys.stdin)['access_token'])")

auth() { curl -sf -o /dev/null -w "%{http_code}" -H "Authorization: Bearer ${TOKEN}" "$@"; }
auth_body() { curl -sf -H "Authorization: Bearer ${TOKEN}" "$@"; }

# 2. GET /auth/me — GDPR fields present
ME=$(auth_body "${API}/auth/me")
echo "$ME" | python3 -c "
import sys, json
d = json.load(sys.stdin)
assert d.get('username'), 'missing username'
assert d.get('email'), 'missing email'
assert isinstance(d.get('email_verified'), bool), 'missing email_verified'
assert d.get('created_at'), 'missing created_at'
" || fail "GET /auth/me — missing GDPR fields"
echo "ok  GET /auth/me (GDPR fields)"

# 3. GET /rooms
check "GET /rooms" "200" "$(auth "${API}/rooms")"

# 4. DELETE /auth/me
check "DELETE /auth/me" "204" "$(curl -sf -o /dev/null -w "%{http_code}" -X DELETE \
  -H "Authorization: Bearer ${TOKEN}" "${API}/auth/me")"

# 5. Token should now be invalid
STATUS=$(curl -s -o /dev/null -w "%{http_code}" -H "Authorization: Bearer ${TOKEN}" "${API}/auth/me")
check "GET /auth/me after delete (expect 401)" "401" "$STATUS"

echo ""
echo "All smoke tests passed."
