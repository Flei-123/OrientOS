#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-2.0-only
set -euo pipefail
cd "$(dirname "$0")"

if [[ $# -lt 1 || $# -gt 2 ]]; then
    echo "usage: ./rename.sh <os-name> [slug]" >&2
    exit 1
fi

OS_NAME_NEW="$1"
SLUG_NEW="${2:-$(printf '%s' "$1" | tr '[:upper:]' '[:lower:]' | sed 's/os$//' | tr -cd 'a-z0-9-')}"

if [[ ! "$OS_NAME_NEW" =~ ^[A-Za-z][A-Za-z0-9-]{0,30}$ ]]; then
    echo "os-name must match ^[A-Za-z][A-Za-z0-9-]{0,30}$: $OS_NAME_NEW" >&2
    exit 1
fi
if [[ ! "$SLUG_NEW" =~ ^[a-z][a-z0-9-]{0,30}$ ]]; then
    echo "slug must match ^[a-z][a-z0-9-]{0,30}$: $SLUG_NEW" >&2
    exit 1
fi
if [[ -e "brands/$SLUG_NEW.toml" ]]; then
    echo "brands/$SLUG_NEW.toml exists already" >&2
    exit 1
fi

OLD_DEFAULT=$(tr -d '[:space:]' < brands/STANDARD)
PUBLISHER=$(sed 's/#.*//' "brands/$OLD_DEFAULT.toml" | grep -m1 '^[[:space:]]*publisher' | sed 's/.*"\(.*\)".*/\1/')

cat > "brands/$SLUG_NEW.toml" <<EOF
# SPDX-License-Identifier: GPL-2.0-only

os-name   = "$OS_NAME_NEW"
slug      = "$SLUG_NEW"
publisher = "${PUBLISHER:-FleiTec}"
web       = "https://$SLUG_NEW.example"
feed      = "https://$SLUG_NEW.example/packages"
EOF
echo "$SLUG_NEW" > brands/STANDARD
echo ">> brands/$SLUG_NEW.toml created, brands/STANDARD now names $SLUG_NEW (was $OLD_DEFAULT)"
echo ">> adjust web and feed in brands/$SLUG_NEW.toml, then run ./build.sh"
