#!/usr/bin/env bash
set -euo pipefail

# ============================================================================
# simse deployment setup
#
# Interactive script that guides you through:
#   1. GitHub Actions secrets (Cloudflare credentials)
#   2. Cloudflare resource creation (D1, KV, R2 for staging + production)
#   3. Wrangler secrets for each worker
#   4. Updating wrangler.toml placeholders with real resource IDs
# ============================================================================

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$REPO_ROOT"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
BOLD='\033[1m'
NC='\033[0m'

header() { echo -e "\n${BLUE}${BOLD}══════════════════════════════════════════${NC}"; echo -e "${BLUE}${BOLD}  $1${NC}"; echo -e "${BLUE}${BOLD}══════════════════════════════════════════${NC}\n"; }
step() { echo -e "${CYAN}→${NC} $1"; }
ok() { echo -e "${GREEN}✓${NC} $1"; }
warn() { echo -e "${YELLOW}!${NC} $1"; }
err() { echo -e "${RED}✗${NC} $1"; }
ask() { echo -en "${BOLD}$1${NC} "; }

confirm() {
    ask "$1 [Y/n]"
    read -r reply
    [[ -z "$reply" || "$reply" =~ ^[Yy] ]]
}

prompt_secret() {
    local var_name="$1" prompt_text="$2"
    ask "$prompt_text"
    read -rs value
    echo
    eval "$var_name='$value'"
}

replace_placeholder() {
    local file="$1" placeholder="$2" value="$3"
    if grep -q "$placeholder" "$file" 2>/dev/null; then
        sed -i "s|$placeholder|$value|g" "$file"
        ok "Updated $file: $placeholder -> $value"
    fi
}

# ============================================================================
header "Prerequisites Check"
# ============================================================================

MISSING=()
command -v gh >/dev/null 2>&1 || MISSING+=("gh (GitHub CLI)")
command -v wrangler >/dev/null 2>&1 || {
    # Try via bunx
    if command -v bunx >/dev/null 2>&1; then
        WRANGLER="bunx wrangler"
    else
        MISSING+=("wrangler (Cloudflare CLI)")
    fi
}
WRANGLER="${WRANGLER:-wrangler}"

command -v jq >/dev/null 2>&1 || MISSING+=("jq (JSON processor)")

if [[ ${#MISSING[@]} -gt 0 ]]; then
    err "Missing required tools:"
    for tool in "${MISSING[@]}"; do
        echo "    - $tool"
    done
    exit 1
fi

ok "gh CLI found"
ok "wrangler found ($WRANGLER)"
ok "jq found"

# Check auth
step "Checking GitHub CLI auth..."
if ! gh auth status >/dev/null 2>&1; then
    err "Not logged in to GitHub CLI. Run: gh auth login"
    exit 1
fi
ok "GitHub CLI authenticated"

step "Checking Wrangler auth..."
if ! $WRANGLER whoami >/dev/null 2>&1; then
    warn "Not logged in to Wrangler. Run: wrangler login"
    if confirm "Run wrangler login now?"; then
        $WRANGLER login
    else
        exit 1
    fi
fi
ok "Wrangler authenticated"

# Detect repo
GH_REPO=$(gh repo view --json nameWithOwner -q .nameWithOwner 2>/dev/null || echo "")
if [[ -z "$GH_REPO" ]]; then
    err "Could not detect GitHub repo. Are you in the right directory?"
    exit 1
fi
ok "Repository: $GH_REPO"

# ============================================================================
header "Step 1: GitHub Actions Secrets"
# ============================================================================

echo "The CI workflow needs these GitHub repo secrets:"
echo "  - CLOUDFLARE_API_TOKEN"
echo "  - CLOUDFLARE_ACCOUNT_ID"
echo

if confirm "Set GitHub secrets now?"; then
    # Account ID
    step "Fetching Cloudflare Account ID..."
    CF_ACCOUNT_ID=$($WRANGLER whoami --json 2>/dev/null | jq -r '.accounts[0].id // empty' || echo "")

    if [[ -n "$CF_ACCOUNT_ID" ]]; then
        ok "Detected Account ID: $CF_ACCOUNT_ID"
        if ! confirm "Use this Account ID?"; then
            ask "Enter Cloudflare Account ID:"
            read -r CF_ACCOUNT_ID
        fi
    else
        ask "Enter Cloudflare Account ID:"
        read -r CF_ACCOUNT_ID
    fi

    echo "$CF_ACCOUNT_ID" | gh secret set CLOUDFLARE_ACCOUNT_ID --repo "$GH_REPO"
    ok "Set CLOUDFLARE_ACCOUNT_ID"

    # API Token
    echo
    echo "Create a Cloudflare API token at:"
    echo "  https://dash.cloudflare.com/profile/api-tokens"
    echo
    echo "Required permissions:"
    echo "  - Account > Workers Scripts > Edit"
    echo "  - Account > Workers KV Storage > Edit"
    echo "  - Account > Workers R2 Storage > Edit"
    echo "  - Account > D1 > Edit"
    echo "  - Account > Cloudflare Pages > Edit"
    echo "  - Account > Workers Tail > Read"
    echo "  - Zone > DNS > Edit (for custom domains)"
    echo

    prompt_secret CF_API_TOKEN "Enter Cloudflare API Token:"
    echo "$CF_API_TOKEN" | gh secret set CLOUDFLARE_API_TOKEN --repo "$GH_REPO"
    ok "Set CLOUDFLARE_API_TOKEN"
else
    warn "Skipped GitHub secrets"
    CF_ACCOUNT_ID=""
fi

# Get account ID if we don't have it yet
if [[ -z "${CF_ACCOUNT_ID:-}" ]]; then
    CF_ACCOUNT_ID=$($WRANGLER whoami --json 2>/dev/null | jq -r '.accounts[0].id // empty' || echo "")
    if [[ -z "$CF_ACCOUNT_ID" ]]; then
        ask "Enter Cloudflare Account ID (needed for resource creation):"
        read -r CF_ACCOUNT_ID
    fi
fi

# ============================================================================
header "Step 2: Create Cloudflare Resources (Staging)"
# ============================================================================

echo "The following staging resources need to be created:"
echo "  - D1: simse-auth-db-staging"
echo "  - D1: simse-payments-db-staging"
echo "  - D1: simse-mailer-db-staging"
echo "  - D1: simse-waitlist-staging"
echo "  - KV:  simse-cdn-staging-version-store"
echo "  - R2:  simse-cdn-staging"
echo

if confirm "Create staging resources now?"; then

    # --- D1 Databases ---
    step "Creating D1 databases..."

    for db_info in \
        "simse-auth-db-staging:simse-auth/wrangler.toml:PLACEHOLDER_STAGING_AUTH_DB_ID" \
        "simse-payments-db-staging:simse-payments/wrangler.toml:PLACEHOLDER_STAGING_PAYMENTS_DB_ID" \
        "simse-mailer-db-staging:simse-mailer/wrangler.toml:PLACEHOLDER_STAGING_MAILER_DB_ID" \
        "simse-waitlist-staging:simse-landing/wrangler.toml:PLACEHOLDER_STAGING_WAITLIST_DB_ID"
    do
        IFS=':' read -r db_name file placeholder <<< "$db_info"
        step "Creating D1 database: $db_name"
        result=$($WRANGLER d1 create "$db_name" --json 2>/dev/null || echo '{}')
        db_id=$(echo "$result" | jq -r '.uuid // empty')
        if [[ -n "$db_id" ]]; then
            ok "Created $db_name (ID: $db_id)"
            replace_placeholder "$file" "$placeholder" "$db_id"
        else
            warn "Could not create $db_name (may already exist)"
            ask "Enter existing database ID for $db_name (or press Enter to skip):"
            read -r db_id
            if [[ -n "$db_id" ]]; then
                replace_placeholder "$file" "$placeholder" "$db_id"
            fi
        fi
    done

    # --- KV Namespace ---
    step "Creating KV namespace: simse-cdn-staging-version-store"
    result=$($WRANGLER kv namespace create "simse-cdn-staging-version-store" --json 2>/dev/null || echo '{}')
    kv_id=$(echo "$result" | jq -r '.id // empty')
    if [[ -n "$kv_id" ]]; then
        ok "Created KV namespace (ID: $kv_id)"
        replace_placeholder "simse-cdn/wrangler.toml" "PLACEHOLDER_STAGING_KV_ID" "$kv_id"
    else
        warn "Could not create KV namespace (may already exist)"
        ask "Enter existing KV namespace ID (or press Enter to skip):"
        read -r kv_id
        if [[ -n "$kv_id" ]]; then
            replace_placeholder "simse-cdn/wrangler.toml" "PLACEHOLDER_STAGING_KV_ID" "$kv_id"
        fi
    fi

    # --- R2 Bucket ---
    step "Creating R2 bucket: simse-cdn-staging"
    if $WRANGLER r2 bucket create simse-cdn-staging 2>/dev/null; then
        ok "Created R2 bucket: simse-cdn-staging"
    else
        warn "Could not create R2 bucket (may already exist)"
    fi

else
    warn "Skipped staging resource creation"
fi

# ============================================================================
header "Step 3: Create Cloudflare Resources (Production)"
# ============================================================================

echo "The following production resources may need to be created:"
echo "  - D1: simse-auth-db (placeholder exists)"
echo "  - D1: simse-mailer-db (placeholder exists)"
echo "  - KV:  simse-cdn-version-store (placeholder exists)"
echo "  - R2:  simse-cdn (may exist)"
echo
echo "Note: simse-payments-db and simse_waitlist already have real IDs."
echo

if confirm "Create production resources now?"; then

    # --- D1 Databases ---
    for db_info in \
        "simse-auth-db:simse-auth/wrangler.toml:PLACEHOLDER_FILL_AFTER_CREATION" \
        "simse-mailer-db:simse-mailer/wrangler.toml:PLACEHOLDER_FILL_AFTER_CREATION"
    do
        IFS=':' read -r db_name file placeholder <<< "$db_info"
        step "Creating D1 database: $db_name"
        result=$($WRANGLER d1 create "$db_name" --json 2>/dev/null || echo '{}')
        db_id=$(echo "$result" | jq -r '.uuid // empty')
        if [[ -n "$db_id" ]]; then
            ok "Created $db_name (ID: $db_id)"
            replace_placeholder "$file" "$placeholder" "$db_id"
        else
            warn "Could not create $db_name (may already exist)"
            ask "Enter existing database ID for $db_name (or press Enter to skip):"
            read -r db_id
            if [[ -n "$db_id" ]]; then
                replace_placeholder "$file" "$placeholder" "$db_id"
            fi
        fi
    done

    # --- KV Namespace ---
    step "Creating KV namespace: simse-cdn-version-store"
    result=$($WRANGLER kv namespace create "simse-cdn-version-store" --json 2>/dev/null || echo '{}')
    kv_id=$(echo "$result" | jq -r '.id // empty')
    if [[ -n "$kv_id" ]]; then
        ok "Created KV namespace (ID: $kv_id)"
        replace_placeholder "simse-cdn/wrangler.toml" "PLACEHOLDER_REPLACE_WITH_KV_ID" "$kv_id"
    else
        warn "Could not create KV namespace (may already exist)"
        ask "Enter existing KV namespace ID (or press Enter to skip):"
        read -r kv_id
        if [[ -n "$kv_id" ]]; then
            replace_placeholder "simse-cdn/wrangler.toml" "PLACEHOLDER_REPLACE_WITH_KV_ID" "$kv_id"
        fi
    fi

    # --- R2 Bucket ---
    step "Creating R2 bucket: simse-cdn"
    if $WRANGLER r2 bucket create simse-cdn 2>/dev/null; then
        ok "Created R2 bucket: simse-cdn"
    else
        warn "Could not create R2 bucket (may already exist)"
    fi

else
    warn "Skipped production resource creation"
fi

# ============================================================================
header "Step 4: Secrets Store IDs"
# ============================================================================

echo "simse-api and simse-mailer use Cloudflare Secrets Store."
echo "You need to create a secrets store in the Cloudflare dashboard"
echo "and provide the store ID."
echo
echo "  Dashboard: https://dash.cloudflare.com > Workers & Pages > Secrets Store"
echo

if confirm "Set Secrets Store IDs now?"; then
    ask "Enter production Secrets Store ID:"
    read -r prod_store_id
    if [[ -n "$prod_store_id" ]]; then
        replace_placeholder "simse-api/wrangler.toml" "PLACEHOLDER_REPLACE_WITH_STORE_ID" "$prod_store_id"
        replace_placeholder "simse-mailer/wrangler.toml" "PLACEHOLDER_REPLACE_WITH_STORE_ID" "$prod_store_id"
    fi

    ask "Enter staging Secrets Store ID (or press Enter to use same as production):"
    read -r staging_store_id
    staging_store_id="${staging_store_id:-$prod_store_id}"
    if [[ -n "$staging_store_id" ]]; then
        replace_placeholder "simse-api/wrangler.toml" "PLACEHOLDER_STAGING_STORE_ID" "$staging_store_id"
        replace_placeholder "simse-mailer/wrangler.toml" "PLACEHOLDER_STAGING_STORE_ID" "$staging_store_id"
    fi
else
    warn "Skipped Secrets Store setup"
fi

# ============================================================================
header "Step 5: Wrangler Secrets (per-worker)"
# ============================================================================

echo "simse-payments requires these wrangler secrets:"
echo "  - STRIPE_SECRET_KEY"
echo "  - STRIPE_WEBHOOK_SECRET"
echo "  - API_SECRET"
echo "  - MAILER_API_URL"
echo "  - MAILER_API_SECRET"
echo

set_worker_secret() {
    local worker="$1" secret_name="$2" env="$3"
    prompt_secret secret_value "  Enter $secret_name for $worker ($env):"
    if [[ -n "$secret_value" ]]; then
        echo "$secret_value" | $WRANGLER secret put "$secret_name" --name "$worker" --env "$env" 2>/dev/null
        ok "Set $secret_name on $worker ($env)"
    else
        warn "Skipped $secret_name"
    fi
}

if confirm "Set simse-payments secrets now?"; then
    for env in staging production; do
        echo
        step "Setting secrets for simse-payments ($env)..."
        for secret in STRIPE_SECRET_KEY STRIPE_WEBHOOK_SECRET API_SECRET MAILER_API_URL MAILER_API_SECRET; do
            set_worker_secret "simse-payments" "$secret" "$env"
        done
    done
else
    warn "Skipped simse-payments secrets"
fi

# ============================================================================
header "Step 6: Verify Remaining Placeholders"
# ============================================================================

step "Scanning for remaining placeholders..."
remaining=$(grep -r "PLACEHOLDER" --include="wrangler.toml" "$REPO_ROOT" 2>/dev/null || true)

if [[ -n "$remaining" ]]; then
    warn "The following placeholders still need to be filled:"
    echo "$remaining" | while IFS= read -r line; do
        echo "    $line"
    done
    echo
    echo "You can re-run this script or manually update these values."
else
    ok "All placeholders have been replaced!"
fi

# ============================================================================
header "Summary"
# ============================================================================

echo "GitHub secrets:"
gh secret list --repo "$GH_REPO" 2>/dev/null | while IFS= read -r line; do
    echo "  $line"
done

echo
echo "Next steps:"
echo "  1. Fill any remaining PLACEHOLDER values in wrangler.toml files"
echo "  2. Run D1 migrations for staging/production databases"
echo "  3. Commit updated wrangler.toml files"
echo "  4. Push to staging branch to trigger first staging deploy"
echo "  5. Push to production branch to trigger first production deploy"
echo

ok "Setup complete!"
