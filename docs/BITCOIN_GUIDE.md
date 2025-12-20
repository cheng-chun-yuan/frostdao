# Bitcoin Taproot Transaction Guide

This guide walks you through sending a Bitcoin testnet transaction using the FrostDAO CLI.

## Prerequisites

```bash
# Build and install globally
cargo install --path .
```

Now you can use `frostdao` directly from anywhere.

---

## Step 1: Check Your Address

First, get your testnet Taproot address:

```bash
frostdao btc-address-testnet
```

**Your Address:**
```
tb1pqjuvav27udjjufh8z8pt6873myjlrgjmelfx62f7xhkl4rrrw5vsw2r2um
```

---

## Step 2: Fund Your Address

Get free testnet BTC from one of these faucets:

| Faucet | URL |
|--------|-----|
| Mempool.space | https://testnet-faucet.mempool.co/ |
| Bitcoin Testnet Faucet | https://bitcoinfaucet.uo1.net/ |
| CoinFaucet | https://coinfaucet.eu/en/btc-testnet/ |

**Instructions:**
1. Copy your testnet address above
2. Go to one of the faucet websites
3. Paste your address and request testnet BTC
4. Wait for confirmation (usually 1-10 minutes)

---

## Step 3: Check Your Balance

After funding, verify your balance:

```bash
frostdao btc-balance
```

Expected output (after funding):
```
Bitcoin Balance Check

━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
Network: testnet
Address: tb1pqjuvav27udjjufh8z8pt6873myjlrgjmelfx62f7xhkl4rrrw5vsw2r2um

Fetching UTXOs from mempool.space...

Total UTXOs: 1
Confirmed UTXOs: 1

Total Balance: 10000 sats (0.00010000 BTC)
Confirmed Balance: 10000 sats (0.00010000 BTC)

UTXO Details:
  10000 sats - abc123...def456:0 (confirmed)
```

**Important:** Wait for at least 1 confirmation before sending.

---

## Step 4: Send Transaction

Send testnet BTC to the recipient:

```bash
frostdao btc-send \
  --to tb1p3e44guscrytuum9q36tlx5kez9zvdheuwxlq9k9y4kud3hyckhtq63fz34 \
  --amount 1000
```

**Parameters:**
| Parameter | Description |
|-----------|-------------|
| `--to` | Recipient's Taproot address |
| `--amount` | Amount in satoshis (1 BTC = 100,000,000 sats) |
| `--fee-rate` | (Optional) Fee rate in sats/vbyte |

**Example with custom fee:**
```bash
frostdao btc-send \
  --to tb1p3e44guscrytuum9q36tlx5kez9zvdheuwxlq9k9y4kud3hyckhtq63fz34 \
  --amount 1000 \
  --fee-rate 2
```

---

## Step 5: Verify Transaction

After broadcasting, you'll receive a transaction ID (txid). View it on the block explorer:

```
https://mempool.space/testnet/tx/<your-txid>
```

---

## Common Amounts Reference

| Amount | Satoshis |
|--------|----------|
| 0.00001 BTC | 1,000 sats |
| 0.0001 BTC | 10,000 sats |
| 0.001 BTC | 100,000 sats |
| 0.01 BTC | 1,000,000 sats |

---

## All Bitcoin Commands

```bash
# Key Management
frostdao btc-keygen              # Generate new keypair
frostdao btc-pubkey              # Show public key
frostdao btc-import-key --secret <hex>  # Import existing key

# Address Generation
frostdao btc-address             # Mainnet address
frostdao btc-address-testnet     # Testnet address
frostdao btc-address-signet      # Signet address

# Balance & Transactions
frostdao btc-balance             # Check testnet balance
frostdao btc-send --to <addr> --amount <sats>  # Send on testnet
frostdao btc-send-signet --to <addr> --amount <sats>  # Send on signet

# Signing & Verification
frostdao btc-sign --message "hello"           # Sign message
frostdao btc-sign-taproot --sighash <hex>     # Sign sighash
frostdao btc-verify --signature <sig> --public-key <pk> --message "hello"
```

---

## Troubleshooting

### "No UTXOs found"
- Your address hasn't been funded yet
- Go to Step 2 and use a faucet

### "No confirmed UTXOs"
- Transactions are still pending
- Wait for 1 block confirmation (~10 minutes on testnet)

### "Insufficient funds"
- Your balance is less than amount + fee
- Reduce the amount or get more testnet BTC

### "Address network mismatch"
- Make sure recipient address starts with `tb1` for testnet
- Mainnet addresses start with `bc1`

### "Broadcast failed"
- Check your internet connection
- Try again in a few seconds
- The raw transaction hex is provided for manual broadcast

---

## Quick Reference

**Your Testnet Address:**
```
tb1pqjuvav27udjjufh8z8pt6873myjlrgjmelfx62f7xhkl4rrrw5vsw2r2um
```

**Recipient Address:**
```
tb1p3e44guscrytuum9q36tlx5kez9zvdheuwxlq9k9y4kud3hyckhtq63fz34
```

**Quick Send Command:**
```bash
frostdao btc-send \
  --to tb1p3e44guscrytuum9q36tlx5kez9zvdheuwxlq9k9y4kud3hyckhtq63fz34 \
  --amount 1000
```

---

## Technical Details

- **Address Type:** P2TR (Pay-to-Taproot, BIP341)
- **Signature:** BIP340 Schnorr
- **Spend Path:** Key-path (tweaked internal key)
- **API:** mempool.space for UTXO fetching and broadcasting
