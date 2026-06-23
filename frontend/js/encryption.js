// NIP-44 E2E encryption utilities
import { nip44 } from 'https://esm.sh/nostr-tools@2.7.2/nip44';
import { bytesToHex, hexToBytes } from 'https://esm.sh/@noble/hashes@1.4.0/utils';

export { nip44, bytesToHex, hexToBytes };

// Encrypt data for a recipient
export function encrypt(data, mySecretHex, recipientPubkeyHex) {
    const conversationKey = nip44.v2.utils.getConversationKey(
        hexToBytes(mySecretHex),
        hexToBytes(recipientPubkeyHex)
    );
    const payload = typeof data === 'string' ? data : JSON.stringify(data);
    return nip44.v2.encrypt(payload, conversationKey);
}

// Decrypt data from a sender
export function decrypt(ciphertext, mySecretHex, senderPubkeyHex) {
    const conversationKey = nip44.v2.utils.getConversationKey(
        hexToBytes(mySecretHex),
        hexToBytes(senderPubkeyHex)
    );
    return nip44.v2.decrypt(ciphertext, conversationKey);
}

// Validate hex string (64 chars = 32 bytes)
export function isValidHex64(str) {
    return /^[0-9a-fA-F]{64}$/.test(str);
}
