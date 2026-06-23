# Product

## Register

product

## Users

FrostDAO is for Bitcoin treasury operators, DAO signers, founders, security teams, and technical family or estate custodians who need shared custody without any one device or person holding the complete private key. Users often work from separate devices and locations, coordinating through copied JSON, Nostr relay messages, or a terminal UI.

## Product Purpose

FrostDAO creates and operates threshold Taproot wallets. A group can create one root wallet, derive many addresses, sign transactions with either standard TSS or rank-aware HTSS, rotate shares through resharing, and recover lost shares without reconstructing the full private key.

Success means a signer can safely understand what step they are in, what they must share, what must stay private, and whether the current signer set is valid before any signature or reshare action proceeds.

## Brand Personality

Precise, sober, technical. The interface should feel like an operator console for high-value custody: clear state, low ambiguity, no decorative noise.

## Anti-references

Avoid marketing-style crypto dashboards, glossy trading-app visuals, gamified wallet flows, hidden protocol state, and one-device assumptions. Avoid copy that implies a relay, browser, or coordinator is trusted with secrets.

## Design Principles

1. Show the ceremony state: users should always know the room, wallet, party index, threshold, scheme, and current round.
2. Separate public from secret: every message should be labeled as public, encrypted, local-only, or broadcast.
3. Make multi-device progress visible: each party should see who has joined, who has sent each round, and what is still missing.
4. Treat TSS and HTSS as equal first-class modes: TSS for peer groups, HTSS for ranked organizations.
5. Preserve custody confidence: warn before irreversible or mainnet actions, and avoid hiding protocol details that affect security.

## Accessibility & Inclusion

Use WCAG AA as the baseline. Important status must not rely on color alone. Terminal and browser flows should support keyboard operation, readable labels, reduced motion, and copyable text for air-gapped or low-connectivity workflows.
