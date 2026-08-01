#!/usr/bin/env python3
"""
Catalog signing utility for ViewIt plugins.
Generates Ed25519 keypair and signs the plugin catalog.

Usage:
  python3 scripts/sign-catalog.py generate
  python3 scripts/sign-catalog.py sign
  python3 scripts/sign-catalog.py sign --input /tmp/catalog.json --output /tmp/catalog.signed.json
  python3 scripts/sign-catalog.py verify
  python3 scripts/sign-catalog.py verify --input /tmp/catalog.signed.json

Keys are stored in:
  - Private: scripts/.catalog-signing-key.priv
  - Public:  scripts/.catalog-signing-key.pub
"""

import base64
import argparse
import json
import sys
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


def ensure_signing_key():
    try:
        from nacl.signing import SigningKey
    except ImportError:
        import subprocess
        subprocess.check_call([sys.executable, "-m", "pip", "install", "pynacl", "--quiet"])
        from nacl.signing import SigningKey
    return SigningKey


def ensure_verify_key():
    try:
        from nacl.signing import VerifyKey
    except ImportError:
        import subprocess
        subprocess.check_call([sys.executable, "-m", "pip", "install", "pynacl", "--quiet"])
        from nacl.signing import VerifyKey
    return VerifyKey


def canonical_json(data):
    return json.dumps(data, separators=(',', ':'), sort_keys=True)


def sign_catalog(input_file=CATALOG_FILE, output_file=SIGNED_CATALOG_FILE, private_key_file=PRIVATE_KEY_FILE):
    """Sign the catalog with the existing private key."""
    if not private_key_file.exists():
        print(f"Error: Private key not found at {private_key_file}")
        print("Run 'generate' first to create a keypair.")
        sys.exit(1)

    if not input_file.exists():
        print(f"Error: Catalog not found at {input_file}")
        sys.exit(1)

    SigningKey = ensure_signing_key()
    private_b64 = private_key_file.read_text().strip()
    signing_key = SigningKey(base64.b64decode(private_b64))

    catalog_data = json.loads(input_file.read_text())
    catalog_json = canonical_json(catalog_data)

    signed = signing_key.sign(catalog_json.encode('utf-8'))
    signature_b64 = base64.b64encode(signed.signature).decode()

    signed_catalog = {
        "catalog": catalog_data,
        "signature": signature_b64
    }

    output_file.parent.mkdir(parents=True, exist_ok=True)
    output_file.write_text(json.dumps(signed_catalog, indent=2) + "\n")

    print(f"Signed catalog: {output_file}")
    print(f"Signature: {signature_b64[:40]}...")


def verify_catalog(input_file=SIGNED_CATALOG_FILE, public_key_file=PUBLIC_KEY_FILE):
    """Verify the signed catalog with the public key."""
    if not public_key_file.exists():
        print(f"Error: Public key not found at {public_key_file}")
        sys.exit(1)

    if not input_file.exists():
        print(f"Error: Signed catalog not found at {input_file}")
        sys.exit(1)

    VerifyKey = ensure_verify_key()
    public_b64 = public_key_file.read_text().strip()
    verify_key = VerifyKey(base64.b64decode(public_b64))

    signed_data = json.loads(input_file.read_text())
    catalog_json = canonical_json(signed_data["catalog"])
    signature_b64 = signed_data["signature"]

    try:
        verify_key.verify(catalog_json.encode('utf-8'), base64.b64decode(signature_b64))
        print(f"✅ Catalog signature is VALID: {input_file}")
    except Exception as e:
        print(f"❌ Catalog signature is INVALID: {e}")
        sys.exit(1)


def parse_args():
    parser = argparse.ArgumentParser(description="Sign and verify ViewIt plugin catalogs")
    subparsers = parser.add_subparsers(dest="command", required=True)

    subparsers.add_parser("generate", help="Generate a new Ed25519 keypair")

    sign_parser = subparsers.add_parser("sign", help="Sign a plugin catalog")
    sign_parser.add_argument("--input", type=Path, default=CATALOG_FILE, help="Unsigned catalog JSON path")
    sign_parser.add_argument("--output", type=Path, default=SIGNED_CATALOG_FILE, help="Signed catalog output path")
    sign_parser.add_argument("--private-key", type=Path, default=PRIVATE_KEY_FILE, help="Base64 Ed25519 private key path")

    verify_parser = subparsers.add_parser("verify", help="Verify a signed plugin catalog")
    verify_parser.add_argument("--input", type=Path, default=SIGNED_CATALOG_FILE, help="Signed catalog JSON path")
    verify_parser.add_argument("--public-key", type=Path, default=PUBLIC_KEY_FILE, help="Base64 Ed25519 public key path")

    return parser.parse_args()


if __name__ == "__main__":
    args = parse_args()
    if args.command == "generate":
        generate_keypair()
    elif args.command == "sign":
        sign_catalog(args.input, args.output, args.private_key)
    elif args.command == "verify":
        verify_catalog(args.input, args.public_key)
