#!/bin/bash
# Semantisk review af staged diff mod projektets guidelines og ADRs.
# Bruges af pre-commit hook og kan køres manuelt eller af en agent.
#
# Brug: scripts/check.sh [--staged]
#   --staged  Tjek kun staged ændringer (bruges af pre-commit hook)
#   (ingen flag)  Tjek working tree mod HEAD
set -euo pipefail

# ── Konfiguration ──────────────────────────────────────────────────────────
LLM_CMD="claude"
LLM_FLAGS="--print"
# open = tillad commit ved LLM-fejl; closed = afvis
LLM_UNAVAILABLE="open"
AGENTS_FILE="AGENTS.md"

# ── Diff ───────────────────────────────────────────────────────────────────
if [ "${1:-}" = "--staged" ]; then
    DIFF=$(git diff --cached)
else
    DIFF=$(git diff HEAD)
fi

if [ -z "$DIFF" ]; then
    exit 0
fi

# ── Find projektrod ────────────────────────────────────────────────────────
GIT_ROOT=$(git rev-parse --show-toplevel)

# ── Læs @-includes fra AGENTS.md ──────────────────────────────────────────
CONTEXT=""

if [ ! -f "$GIT_ROOT/$AGENTS_FILE" ]; then
    echo "check.sh: ingen $AGENTS_FILE fundet — springer semantisk review over" >&2
    exit 0
fi

while IFS= read -r line; do
    [[ "$line" =~ ^@(.+) ]] || continue
    raw_path="${BASH_REMATCH[1]}"
    # Ekspandér ~ og løs relative stier fra projektroden
    if [[ "$raw_path" == "~/"* ]]; then
        full_path="${HOME}/${raw_path:2}"
    else
        full_path="$GIT_ROOT/$raw_path"
    fi
    if [ -f "$full_path" ]; then
        filename=$(basename "$full_path")
        CONTEXT+="### ${filename}\n\n$(cat "$full_path")\n\n---\n\n"
    else
        echo "check.sh: advarsel — kan ikke læse '$full_path'" >&2
    fi
done < "$GIT_ROOT/$AGENTS_FILE"

if [ -z "$CONTEXT" ]; then
    echo "check.sh: ingen @-includes fundet i $AGENTS_FILE — springer semantisk review over" >&2
    exit 0
fi

# ── Tjek at LLM er tilgængeligt ───────────────────────────────────────────
if ! command -v "$LLM_CMD" &>/dev/null; then
    echo "check.sh: ⚠ '$LLM_CMD' ikke fundet — springer review over (fail open)" >&2
    exit 0
fi

# ── Byg prompt og kald LLM ────────────────────────────────────────────────
PROMPT="Du er en code reviewer der tjekker om et git commit overholder projektets guidelines og ADRs.

## Staged diff

\`\`\`diff
${DIFF}
\`\`\`

## Projektets guidelines og ADRs

${CONTEXT}

## Opgave

Gennemgå diff'en og identificér konkrete overtrædelser af de listede guidelines og ADRs.
- Henvis til specifikke linjer i diff'en og den præcise regel der brydes.
- Ignorer forhold der ikke er dækket af de listede dokumenter.
- Vær kortfattet. Én linje per concern.

Svar udelukkende i ét af disse to formater:

VERDICT: APPROVED

eller:

VERDICT: CONCERNS
- [fil:linje]: [overtrædelse] (ref: [dokumentnavn])"

TMPFILE=$(mktemp)
trap 'rm -f "$TMPFILE"' EXIT
printf '%s' "$PROMPT" > "$TMPFILE"

echo "" >&2
echo "check.sh: kører semantisk review…" >&2

if ! RESPONSE=$($LLM_CMD $LLM_FLAGS "$(cat "$TMPFILE")" 2>/dev/null); then
    echo "check.sh: ⚠ LLM-kald fejlede — springer review over (fail open)" >&2
    exit 0
fi

# ── Vis resultat og afgør exit-kode ───────────────────────────────────────
echo ""
echo "── Semantic review ──────────────────────────────────────────────────────"
echo "$RESPONSE"
echo "─────────────────────────────────────────────────────────────────────────"
echo ""

if echo "$RESPONSE" | grep -q "^VERDICT: APPROVED"; then
    exit 0
else
    echo "check.sh: commit afvist — ret ovenstående og forsøg igen" >&2
    echo "check.sh: brug 'git commit --no-verify' for at springe review over" >&2
    exit 1
fi
