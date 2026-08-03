#!/usr/bin/env python3
"""Minimal STUN binding client for the REACH-02 NAT-type test.

Sends a STUN Binding Request to several servers FROM THE SAME local UDP
socket and prints the reflexive (public) address each one reports.

  same public port from every server  -> cone NAT      (hole-punching could work)
  different public port per server    -> symmetric NAT (hole-punching can never work)

Using one socket for all servers is the whole point: symmetric NAT allocates a
new external port per destination, so the mapping differs only if you keep the
local port fixed and vary the peer.
"""
import secrets
import socket
import struct
import sys

MAGIC = 0x2112A442
BINDING_REQUEST = 0x0001
MAPPED_ADDRESS = 0x0001
XOR_MAPPED_ADDRESS = 0x0020

SERVERS = [
    ("stun.l.google.com", 19302),
    ("stun1.l.google.com", 19302),
    ("stun2.l.google.com", 19302),
    ("stun.cloudflare.com", 3478),
]


def parse(data: bytes):
    """Return (ip, port) from the first MAPPED / XOR-MAPPED attribute."""
    if len(data) < 20:
        return None
    _typ, length = struct.unpack("!HH", data[0:4])
    body, off = data[20 : 20 + length], 0
    while off + 4 <= len(body):
        attr, alen = struct.unpack("!HH", body[off : off + 4])
        val = body[off + 4 : off + 4 + alen]
        if attr in (XOR_MAPPED_ADDRESS, MAPPED_ADDRESS) and len(val) >= 8:
            port = struct.unpack("!H", val[2:4])[0]
            raw = val[4:8]
            if attr == XOR_MAPPED_ADDRESS:
                port ^= MAGIC >> 16
                raw = bytes(b ^ c for b, c in zip(raw, struct.pack("!I", MAGIC)))
            return socket.inet_ntoa(raw), port
        off += 4 + alen + ((4 - alen % 4) % 4)  # attributes are 4-byte aligned
    return None


def main():
    sock = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
    sock.settimeout(5)
    sock.bind(("", 0))
    print(f"local UDP port (fixed for every probe): {sock.getsockname()[1]}\n")

    results = []
    for host, port in SERVERS:
        req = struct.pack("!HHI", BINDING_REQUEST, 0, MAGIC) + secrets.token_bytes(12)
        try:
            sock.sendto(req, (host, port))
            mapped = parse(sock.recv(2048))
        except (socket.timeout, socket.gaierror, OSError) as exc:
            print(f"  {host:<24} -> no reply ({type(exc).__name__})")
            continue
        if not mapped:
            print(f"  {host:<24} -> reply had no mapped address")
            continue
        print(f"  {host:<24} -> {mapped[0]}:{mapped[1]}")
        results.append(mapped)

    if len(results) < 2:
        print("\nINCONCLUSIVE: need at least two successful probes.")
        return 2

    ports = {p for _, p in results}
    ips = {i for i, _ in results}
    print(f"\npublic IPs seen : {sorted(ips)}")
    print(f"public ports    : {sorted(ports)}")
    if len(ports) == 1:
        print("\nRESULT: CONE NAT — same external port to every peer.")
        print("        Hole-punching could work on this network.")
    else:
        print("\nRESULT: SYMMETRIC NAT — a different external port per peer.")
        print("        Hole-punching can never work here. Relay is mandatory.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
