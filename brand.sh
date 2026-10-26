#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-2.0-only

brand_field() {
    { sed 's/#.*//' "$1" 2>/dev/null \
        | grep -m1 "^[[:space:]]*$2[[:space:]]*=" \
        | sed 's/.*=[[:space:]]*"\(.*\)".*/\1/'; } || true
}

BRAND="${BRAND:-}"
if [[ -z "$BRAND" ]]; then
    BRAND=$(sed 's/#.*//' brands/STANDARD 2>/dev/null | tr -d '[:space:]')
fi
BRAND_FILE="brands/${BRAND}.toml"
if [[ ! -f "$BRAND_FILE" ]]; then
    echo "unknown brand: ${BRAND:-<none>}" >&2
    echo "available: $(ls brands/*.toml 2>/dev/null | xargs -r -n1 basename | sed 's/\.toml$//' | tr '\n' ' ')" >&2
    exit 1
fi
export BRAND

OS_NAME=$(brand_field "$BRAND_FILE" os-name)
SLUG=$(brand_field "$BRAND_FILE" slug)
PUBLISHER=$(brand_field "$BRAND_FILE" publisher)
WEB=$(brand_field "$BRAND_FILE" web)
FEED=$(brand_field "$BRAND_FILE" feed)
KERNEL_PKG=$(brand_field "$BRAND_FILE" kernel-name)

if [[ -z "${OS_NAME:-}" || -z "${SLUG:-}" ]]; then
    echo "$BRAND_FILE: os-name and slug are required" >&2
    exit 1
fi
if [[ ! "$SLUG" =~ ^[a-z][a-z0-9-]*$ ]]; then
    echo "$BRAND_FILE: slug \"$SLUG\" is not usable as a file name" >&2
    exit 1
fi
[[ -n "${KERNEL_PKG:-}" ]] || KERNEL_PKG=osum

export OS_NAME SLUG PUBLISHER WEB FEED KERNEL_PKG

export OSUM_MARKE_PRODUKT="$OS_NAME"
export OSUM_MARKE_KURZ="$SLUG"
[[ -n "$PUBLISHER" ]] && export OSUM_MARKE_HERSTELLER="$PUBLISHER" || unset OSUM_MARKE_HERSTELLER
[[ -n "$WEB" ]]       && export OSUM_MARKE_WEB="$WEB"              || unset OSUM_MARKE_WEB
[[ -n "$FEED" ]]      && export OSUM_MARKE_FEED="$FEED"            || unset OSUM_MARKE_FEED
[[ -n "$(brand_field "$BRAND_FILE" kernel-name)" ]] \
    && export OSUM_MARKE_KERN="$KERNEL_PKG" || unset OSUM_MARKE_KERN
