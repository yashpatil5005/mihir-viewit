#!/usr/bin/env python3
"""
Catalog signing utility for ViewIt plugins.
Generates Ed25519 keypair and signs the plugin catalog.

Usage:
  python3 scripts/sign-catalog.py generate          # Generate new keypair
  python3 scripts/sign-catalog.py sign              # Sign catalog with existing key
  python3 scripts/sign-catalog.py verify            # Verify catalog signature

Keys are stored in:
  - Private: scripts/.catalog-signing-key.priv
  - Public:  scripts/.catalog-signing-key.pub
"""

import json
import base64
import sys
import os
from pathlib import Path

SCRIPT_DIR = Path(__file__).parent
PRIVATE_KEY_FILE = SCRIPT_DIR / ".catalog-signing-key.priv"
PUBLIC_KEY_FILE = SCRIPT_DIR / ".catalog-signing-key.pub"
CATALOG_FILE = SCRIPT_DIR.parent / "plugins" / "catalog.json"
SIGNED_CATALOG_FILE = SCRIPT_DIR.parent / "plugins" / "catalog.signed.json"


def generate_keypair():
    """Generate a new Ed25519 keypair."""
    try:
        from nacl.signing import SigningKey
    except ImportError:
        print("Installing PyNaCl...")
        import subprocess
        subprocess.check_call([sys.executable, "-m", "pip", "install", "pynacl", "--quiet"])
        from nacl.signing import SigningKey

    signing_key = SigningKey.generate()
    verify_key = signing_key.verify_key

    private_b64 = base64.b64encode(signing_key.encode()).decode()
    public_b64 = base64.b64encode(verify_key.encode()).decode()

    PRIVATE_KEY_FILE.write_text(private_b64)
    PUBLIC_KEY_FILE.write_text(public_b64)

    print(f"Private key: {PRIVATE_KEY_FILE}")
    print(f"Public key:  {PUBLIC_KEY_FILE}")
    print(f"\nPublic key (for PluginManager.kt):")
    print(f'  private const val CATALOG_PUBLIC_KEY_B64 = "{public_b64}"')
    print(f"\n⚠️  Store the private key securely. Do NOT commit it to git!")
    print(f"   (scripts/.catalog-signing-key.priv is in .gitignore)")


def sign_catalog():
    """Sign the catalog with the existing private key."""
    if not PRIVATE_KEY_FILE.exists():
        print(f"Error: Private key not found at {PRIVATE_KEY_FILE}")
        print("Run 'generate' first to create a keypair.")
        sys.exit(1)

    if not CATALOG_FILE.exists():
        print(f"Error: Catalog not found at {CATALOG_FILE}")
        sys.exit(1)

    try:
        from nacl.signing import SigningKey
    except ImportError:
        import subprocess
        subprocess.check_call([sys.executable, "-m", "pip", "install", "pynacl", "--quiet"])
        from nacl.signing import SigningKey

    private_b64 = PRIVATE_KEY_FILE.read_text().strip()
    signing_key = SigningKey(base64.b64decode(private_b64))

    catalog_data = json.loads(CATALOG_FILE.read_text())
    catalog_json = json.dumps(catalog_data, separators=(',', ':'), sort_keys=True)

    signed = signing_key.sign(catalog_json.encode('utf-8'))
    signature_b64 = base64.b64encode(signed.signature).decode()

    signed_catalog = {
        "catalog": catalog_data,
        "signature": signature_b64
    }

    SIGNED_CATALOG_FILE.write_text(json.dumps(signed_catalog, indent=2) + "\n")

    print(f"Signed catalog: {SIGNED_CATALOG_FILE}")
    print(f"Signature: {signature_b64[:40]}...")


def verify_catalog():
    """Verify the signed catalog with the public key."""
    if not PUBLIC_KEY_FILE.exists():
        print(f"Error: Public key not found at {PUBLIC_KEY_FILE}")
        sys.exit(1)

    if not SIGNED_CATALOG_FILE.exists():
        print(f"Error: Signed catalog not found at {SIGNED_CATALOG_FILE}")
        sys.exit(1)

    try:
        from nacl.signing import VerifyKey
    except ImportError:
        import subprocess
        subprocess.check_call([sys.executable, "-m", "pip", "install", "pynacl", "--quiet"])
        from nacl.signing import VerifyKey

    public_b64 = PUBLIC_KEY_FILE.read_text().strip()
    verify_key = VerifyKey(base64.b64decode(public_b64))

    signed_data = json.loads(SIGNED_CATALOG_FILE.read_text())
    catalog_json = json.dumps(signed_data["catalog"], separators=(',', ':'), sort_keys=True)
    signature_b64 = signed_data["signature"]

    try:
        verify_key.verify(catalog_json.encode('utf-8'), base64.b64decode(signature_b64))
        print("✅ Catalog signature is VALID")
    except Exception as e:
        print(f"❌ Catalog signature is INVALID: {e}")
        sys.exit(1)


def print_usage():
    print(__doc__)
    print("Commands:")
    print("  generate  - Generate a new Ed25519 keypair")
    print("  sign      - Sign the catalog")
    print("  verify    - Verify the signed catalog")


if __name__ == "__main__":
    if len(sys.argv) < 2:
        print_usage()
        sys.exit(1)

    command = sys.argv[1].lower()
    if command == "generate":
        generate_keypair()
    elif command == "sign":
        sign_catalog()
    elif command == "verify":
        verify_catalog()
    else:
        print(f"Unknown command: {command}")
        print_usage()
        sys.exit(1)
