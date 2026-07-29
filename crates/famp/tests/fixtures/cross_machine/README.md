# cross_machine fixture certs

Self-signed RSA-2048 leaf certificates for `crates/famp-gateway/tests/e2e_cross_host_delivery.rs`
(loopback two-process cross-host relay E2E). SANs are `127.0.0.1` and `localhost`
because the E2E binds `127.0.0.1` for both gateway processes.

**Policy (D-08, cross-platform):** `rustls-platform-verifier` delegates to
Apple SecTrust on macOS and webpki on Linux, and the two verifiers diverge on
EKU/basicConstraints enforcement (Apple rejects a cert with no
`extendedKeyUsage`; webpki rejects a cert whose `basicConstraints` marks it
`CA:TRUE` when used as a leaf). The recipe below — `basicConstraints=critical,
CA:FALSE` + `extendedKeyUsage=serverAuth` — satisfies both verifiers.

Regenerate with:

```bash
for h in alice bob; do
  openssl req -x509 -newkey rsa:2048 -nodes -days 800 \
    -keyout "${h}.key" -out "${h}.crt" -subj "/CN=${h}" \
    -addext "subjectAltName=IP:127.0.0.1,DNS:localhost" \
    -addext "basicConstraints=critical,CA:FALSE" \
    -addext "keyUsage=critical,digitalSignature,keyEncipherment" \
    -addext "extendedKeyUsage=serverAuth"
done
```

Verify with `openssl x509 -in alice.crt -text -noout` — confirm `Basic
Constraints: critical / CA:FALSE` and `Extended Key Usage: TLS Web Server
Authentication` are both present.

These are committed for CI reproducibility but have no long-term guarantee —
regenerate (same recipe) if any test starts failing due to cert validity or
issuer changes. The 800-day validity window means these will eventually
expire; regenerate then too.
