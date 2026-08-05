#!/bin/sh
# Read-only, fail-closed predicate for a Phase 20 clean release-install host.

set -eu

FAIL=0
OS=$(uname -s 2>/dev/null || printf '%s' unknown)
ARCH=$(uname -m 2>/dev/null || printf '%s' unknown)
UTC=$(date -u '+%Y-%m-%dT%H:%M:%SZ' 2>/dev/null || printf '%s' unavailable)

printf 'OS: %s\n' "$OS"
printf 'ARCH: %s\n' "$ARCH"
printf 'UTC: %s\n' "$UTC"
printf 'HOME: <HOME>\n'

case "$OS:$ARCH" in
    Darwin:arm64|Darwin:aarch64|Darwin:x86_64|Linux:x86_64) ;;
    *)
        printf 'UNSUPPORTED PLATFORM: %s/%s has no published Phase 20 release path\n' "$OS" "$ARCH" >&2
        FAIL=1
        ;;
esac

for binary in rustc cargo famp famp-gateway; do
    if command -v "$binary" >/dev/null 2>&1; then
        printf 'CONTAMINATION: %s is already present on PATH; use a genuinely unprepared host\n' "$binary" >&2
        FAIL=1
    fi
done

if [ "${FAMP_HOME+x}" = x ]; then
    printf 'CONTAMINATION: FAMP_HOME is set; unset it and use a genuinely unprepared host\n' >&2
    FAIL=1
fi

DEFAULT_STATE=${HOME}/.famp
if [ -e "$DEFAULT_STATE" ]; then
    printf 'CONTAMINATION: default FAMP state exists under <HOME>/.famp\n' >&2
    FAIL=1
fi

for endpoint in "${HOME}/.famp/bus.sock" "${HOME}/.famp/broker.sock"; do
    if [ -e "$endpoint" ] || [ -S "$endpoint" ]; then
        printf 'CONTAMINATION: broker/socket exists under <HOME>/.famp\n' >&2
        FAIL=1
        break
    fi
done

# Tests select the service manager explicitly; production selects by OS.
SERVICE_CHECK=${PHASE20_SERVICE_CHECK:-}
if [ -z "$SERVICE_CHECK" ]; then
    case "$OS" in
        Darwin) SERVICE_CHECK=launchctl ;;
        Linux) SERVICE_CHECK=systemctl ;;
        *) SERVICE_CHECK=none ;;
    esac
fi

case "$SERVICE_CHECK" in
    systemctl)
        if command -v systemctl >/dev/null 2>&1 && systemctl --user is-active --quiet famp-broker.service >/dev/null 2>&1; then
            printf 'CONTAMINATION: service famp-broker.service is active\n' >&2
            FAIL=1
        fi
        ;;
    launchctl)
        if command -v launchctl >/dev/null 2>&1 && launchctl print "gui/$(id -u)/com.famp.broker" >/dev/null 2>&1; then
            printf 'CONTAMINATION: service com.famp.broker is loaded\n' >&2
            FAIL=1
        fi
        ;;
    none) ;;
    *)
        printf 'UNSUPPORTED PLATFORM: unknown service check %s\n' "$SERVICE_CHECK" >&2
        FAIL=1
        ;;
esac

if [ "$FAIL" -eq 0 ]; then
    printf 'CLEAN HOST: PASS\n'
else
    printf 'CLEAN HOST: FAIL\n' >&2
fi

exit "$FAIL"
