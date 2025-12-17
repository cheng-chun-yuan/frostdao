#!/usr/bin/env python3
"""
HTSS Interactive Demo Server
Simple HTTP server that wraps the yushan CLI for browser interaction
"""

import http.server
import json
import os
import subprocess
import urllib.parse
from pathlib import Path

SCRIPT_DIR = Path(__file__).parent.absolute()
YUSHAN = SCRIPT_DIR / "target" / "release" / "yushan"
BASE = Path("/tmp/htss_interactive")

# Party configuration - ranks 0,1,1,2,2
PARTIES = [
    {"id": "ceo", "index": 1, "rank": 0, "name": "CEO"},
    {"id": "cfo", "index": 2, "rank": 1, "name": "CFO"},
    {"id": "coo", "index": 3, "rank": 1, "name": "COO"},
    {"id": "director", "index": 4, "rank": 2, "name": "Director"},
    {"id": "manager", "index": 5, "rank": 2, "name": "Manager"},
]
THRESHOLD = 3
N_PARTIES = 5

# State
state = {
    "dkg_done": False,
    "public_key": "",
    "party_data": {},
}


def run_cmd(args, cwd=None):
    """Run yushan command and return output"""
    result = subprocess.run(
        [str(YUSHAN)] + args,
        capture_output=True,
        text=True,
        cwd=cwd
    )
    return result.stdout + result.stderr


def do_dkg():
    """Run full DKG for all parties"""
    global state

    # Clean start
    if BASE.exists():
        import shutil
        shutil.rmtree(BASE)
    BASE.mkdir(parents=True)

    logs = []

    # Round 1
    logs.append("=== ROUND 1: Generate Commitments ===")
    r1_outputs = []
    for p in PARTIES:
        party_dir = BASE / p["id"]
        party_dir.mkdir(exist_ok=True)

        output = run_cmd([
            "keygen-round1",
            "--threshold", str(THRESHOLD),
            "--n-parties", str(N_PARTIES),
            "--my-index", str(p["index"]),
            "--rank", str(p["rank"]),
            "--hierarchical"
        ], cwd=party_dir)

        # Extract JSON result
        for line in output.split('\n'):
            if '"party_index"' in line:
                r1_outputs.append(line.strip())
                break

        logs.append(f"  {p['name']}: commitments generated (rank {p['rank']})")

    # Round 2
    logs.append("=== ROUND 2: Exchange Secret Shares ===")
    r1_data = " ".join(r1_outputs)
    r2_outputs = []
    for p in PARTIES:
        party_dir = BASE / p["id"]

        output = run_cmd([
            "keygen-round2",
            "--data", r1_data
        ], cwd=party_dir)

        for line in output.split('\n'):
            if '"party_index"' in line:
                r2_outputs.append(line.strip())
                break

        logs.append(f"  {p['name']}: secret shares computed")

    # Finalize
    logs.append("=== FINALIZE: Compute Final Keys ===")
    r2_data = " ".join(r2_outputs)
    public_key = ""
    for p in PARTIES:
        party_dir = BASE / p["id"]

        output = run_cmd([
            "keygen-finalize",
            "--data", r2_data
        ], cwd=party_dir)

        # Extract public key
        for line in output.split('\n'):
            if "Public Key:" in line:
                public_key = line.split("Public Key:")[1].strip()
                break

        logs.append(f"  {p['name']}: finalized")

    state["dkg_done"] = True
    state["public_key"] = public_key
    logs.append("")
    logs.append(f"Public Key: {public_key}")
    logs.append("DKG Complete!")

    return {
        "success": True,
        "public_key": public_key,
        "logs": logs
    }


def do_verify(signature, public_key, message):
    """Verify a signature"""
    output = run_cmd([
        "verify",
        "--signature", signature,
        "--public-key", public_key,
        "--message", message
    ])

    is_valid = "VALID" in output and "INVALID" not in output

    return {
        "success": True,
        "valid": is_valid,
        "signature": signature,
        "public_key": public_key,
        "message": message,
        "output": output
    }


def do_sign(signer_ids, message):
    """Sign with selected parties"""
    global state

    if not state["dkg_done"]:
        return {"success": False, "error": "Run DKG first"}

    # Validate signers
    signers = [p for p in PARTIES if p["id"] in signer_ids]
    if len(signers) != THRESHOLD:
        return {"success": False, "error": f"Need exactly {THRESHOLD} signers"}

    ranks = sorted([p["rank"] for p in signers])

    # Check HTSS validity (but don't reject - let user see verification fail)
    valid = True
    checks = []
    for i, r in enumerate(ranks):
        ok = r <= i
        checks.append({"pos": i, "rank": r, "pass": ok})
        if not ok:
            valid = False

    logs = []
    logs.append(f"Signers: {[p['name'] for p in signers]}")
    logs.append(f"Ranks (sorted): {ranks}")
    logs.append(f"HTSS Validation: {'VALID' if valid else 'INVALID - will fail at verification!'}")
    for c in checks:
        symbol = "✓" if c["pass"] else "✗"
        logs.append(f"  {symbol} rank {c['rank']} {'<=' if c['pass'] else '>'} position {c['pos']}")

    # Generate nonces
    session = f"sign_{os.urandom(4).hex()}"
    logs.append("")
    logs.append("=== Generating Nonces ===")
    nonces = []
    for p in signers:
        party_dir = BASE / p["id"]
        output = run_cmd([
            "generate-nonce",
            "--session", session
        ], cwd=party_dir)

        for line in output.split('\n'):
            if '"party_index"' in line:
                nonces.append(line.strip())
                break
        logs.append(f"  {p['name']}: nonce generated")

    # Create signature shares
    logs.append("=== Creating Signature Shares ===")
    nonce_data = " ".join(nonces)
    shares = []
    htss_error = None
    for p in signers:
        party_dir = BASE / p["id"]
        output = run_cmd([
            "sign",
            "--session", session,
            "--message", message,
            "--data", nonce_data
        ], cwd=party_dir)

        # Check for HTSS error - but continue to create dummy signature
        # Note: Only check for actual error message, not "Birkhoff" which appears in normal output
        if "Invalid HTSS signer set" in output:
            htss_error = output
            logs.append(f"  {p['name']}: HTSS validation failed!")
            break

        for line in output.split('\n'):
            if '"party_index"' in line:
                shares.append(line.strip())
                break
        logs.append(f"  {p['name']}: signature share created")

    # If HTSS failed, create dummy signature for verification demo
    if htss_error:
        logs.append("")
        logs.append("=== HTSS REJECTED - Creating invalid signature for demo ===")
        # Create a dummy signature (all zeros) - will fail verification
        dummy_sig = "00" * 64  # 64 bytes = 128 hex chars
        dummy_pubkey = "00" * 32  # 32 bytes = 64 hex chars
        logs.append("  Created dummy signature (will fail verification)")
        logs.append("")
        logs.append("Try verifying this signature - it will fail!")

        return {
            "success": True,
            "valid": False,
            "htss_rejected": True,
            "signers": [p["name"] for p in signers],
            "ranks": ranks,
            "checks": checks,
            "message": message,
            "signature": dummy_sig,
            "public_key": dummy_pubkey,
            "public_key_compressed": state["public_key"],
            "error": "HTSS validation failed - signature is invalid",
            "logs": logs
        }

    # Combine
    logs.append("=== Combining Signature ===")
    share_data = " ".join(shares)
    party_dir = BASE / signers[0]["id"]
    output = run_cmd([
        "combine",
        "--data", share_data
    ], cwd=party_dir)

    signature = ""
    for line in output.split('\n'):
        if line.startswith("Signature:"):
            signature = line.split("Signature:")[1].strip()
            break

    # Extract public key from combine output (x-only format, no prefix)
    pubkey_from_combine = ""
    for line in output.split('\n'):
        if line.startswith("Public Key:"):
            pubkey_from_combine = line.split("Public Key:")[1].strip()
            break

    logs.append(f"  Final Signature: {signature[:32]}...")
    logs.append(f"  Public Key (x-only): {pubkey_from_combine[:32]}...")
    logs.append("")
    logs.append("Signature created successfully!")

    return {
        "success": True,
        "valid": True,
        "signers": [p["name"] for p in signers],
        "ranks": ranks,
        "checks": checks,
        "message": message,
        "signature": signature,
        "public_key": pubkey_from_combine,  # Use x-only format for verification
        "public_key_compressed": state["public_key"],  # Keep compressed format too
        "logs": logs
    }


class DemoHandler(http.server.SimpleHTTPRequestHandler):
    def do_GET(self):
        parsed = urllib.parse.urlparse(self.path)

        if parsed.path == "/api/status":
            self.send_json({
                "dkg_done": state["dkg_done"],
                "public_key": state["public_key"],
                "parties": PARTIES,
                "threshold": THRESHOLD
            })
        elif parsed.path == "/api/dkg":
            result = do_dkg()
            self.send_json(result)
        elif parsed.path.startswith("/api/sign"):
            params = urllib.parse.parse_qs(parsed.query)
            signers = params.get("signers", [""])[0].split(",")
            message = params.get("message", ["Hello HTSS"])[0]
            result = do_sign(signers, message)
            self.send_json(result)
        elif parsed.path.startswith("/api/verify"):
            params = urllib.parse.parse_qs(parsed.query)
            signature = params.get("signature", [""])[0]
            public_key = params.get("public_key", [""])[0]
            message = params.get("message", [""])[0]
            result = do_verify(signature, public_key, message)
            self.send_json(result)
        else:
            # Serve static files
            super().do_GET()

    def send_json(self, data):
        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.send_header("Access-Control-Allow-Origin", "*")
        self.end_headers()
        self.wfile.write(json.dumps(data).encode())

    def log_message(self, format, *args):
        # Quieter logging
        try:
            if args and "/api/" in str(args[0]):
                print(f"API: {args[0]}")
        except:
            pass


def main():
    os.chdir(SCRIPT_DIR)
    port = 8888

    print(f"""
╔════════════════════════════════════════════════════════════╗
║           HTSS Interactive Demo Server                     ║
╠════════════════════════════════════════════════════════════╣
║  Open in browser: http://localhost:{port}/demo-interactive.html
║                                                            ║
║  Parties: CEO (r0), CFO (r1), COO (r1), Manager (r2)      ║
║  Threshold: 2-of-4                                         ║
║                                                            ║
║  Press Ctrl+C to stop                                      ║
╚════════════════════════════════════════════════════════════╝
""")

    server = http.server.HTTPServer(("", port), DemoHandler)
    server.serve_forever()


if __name__ == "__main__":
    main()
