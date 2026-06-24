# Backup Guide

FrostDAO backups are per-party backups. A backup must never contain the full group private key.

## What To Back Up

Each party should store:

- the 24-word share mnemonic
- the public backup manifest printed by `dkg-generate-mnemonic`
- wallet name
- party index
- rank, for HTSS
- threshold and total party count
- group public key, public party-rank map, public shared-key polynomial, and address fingerprint
- recovery instructions

The mnemonic is secret. The manifest is public metadata that helps verify the mnemonic belongs to the correct wallet and party. Its party-rank map and shared-key polynomial are restore metadata for the FROST wallet, not private shares.

## Generate A Backup

After `keygen-finalize`, run:

```bash
frostdao dkg-generate-mnemonic --name treasury
```

The command prints:

- a 24-word BIP-39 mnemonic for the local secret share
- a public JSON backup manifest
- a backup ID

Store the mnemonic offline. Store the manifest with it or in a separate inventory system.

## Verify A Backup

Before relying on a backup, verify the written mnemonic against the local wallet:

```bash
frostdao dkg-verify-mnemonic --name treasury --words '<24 words>'
```

The command checks:

- BIP-39 checksum and wordlist
- 32-byte share entropy
- wallet metadata binding
- party index and rank binding
- group public key binding
- backup ID integrity
- share fingerprint match

Verification does not publish or restore the share.

## Storage Rules

- Keep the mnemonic offline.
- Do not paste the mnemonic into Nostr, chat, email, issue trackers, or shared notes.
- Do not store all party mnemonics in one place.
- Store at least one copy in a tamper-evident physical location.
- For organizations, require two-person access to backup storage.
- Run a testnet or signet backup verification drill before mainnet use.

## Lost Device Path

If a device is lost but the party has their mnemonic:

1. Rebuild or reinstall FrostDAO.
2. Verify the mnemonic against the public backup manifest.
3. Restore the local wallet files:

```bash
frostdao dkg-restore-mnemonic --name treasury --words '<24 words>' --manifest backup-manifest.json
```

4. Verify the wallet address against the manifest.
5. Sign a small testnet/signet transaction before using mainnet.

If the mnemonic is lost too:

1. Use `recover-round1` and `recover-finalize` with helper parties.
2. Verify the recovered wallet address matches the original.
3. Reshare immediately to rotate away from the lost setup.

## Current Status

Implemented:

- share mnemonic generation
- public backup manifest
- public party-rank map in the manifest
- public shared-key polynomial in the manifest
- backup ID
- share fingerprint
- mnemonic verification against local wallet metadata
- mnemonic restore/import into a missing local wallet directory

Still required before relying on mnemonic-only disaster recovery:

- encrypted backup file export/import
- recovery drill automation
