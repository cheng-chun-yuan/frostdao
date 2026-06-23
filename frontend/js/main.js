// Main entry point - wires up all modules
import * as rooms from './rooms.js';
import * as ui from './ui.js';
import * as encryption from './encryption.js';

// DKG Room Actions
window.dkgUpdateConfig = () => {
    const roomId = document.getElementById('dkg-room-id').value.trim();
    const myIndex = parseInt(document.getElementById('dkg-my-index').value);
    rooms.rooms.dkg.myIndex = myIndex;

    document.getElementById('dkg-status-text').textContent = 'Connecting...';
    rooms.subscribeDkg('dkg', roomId, (event) => {
        if (event === 'connected') ui.updateConnectionStatus('dkg', true);
        ui.refreshDkgUI('dkg');
    });
};

window.htssUpdateConfig = () => {
    const roomId = document.getElementById('htss-room-id').value.trim();
    const myIndex = parseInt(document.getElementById('htss-my-index').value);
    rooms.rooms.htss.myIndex = myIndex;

    document.getElementById('htss-status-text').textContent = 'Connecting...';
    rooms.subscribeDkg('htss', roomId, (event) => {
        if (event === 'connected') ui.updateConnectionStatus('htss', true);
        ui.refreshDkgUI('htss');
    });
};

window.dkgClearAll = () => {
    rooms.rooms.dkg.data.keygen_round1.clear();
    rooms.rooms.dkg.data.keygen_round2.clear();
    ui.refreshDkgUI('dkg');
};

// Post to Nostr room
window.postToNostr = async (textareaId, expectedType, prefix) => {
    const textarea = document.getElementById(textareaId);
    const content = textarea.value.trim();

    if (!content) { alert('Paste CLI output first'); return; }
    if (!rooms.rooms[prefix].room) { alert('Subscribe to a room first'); return; }

    let data;
    try { data = JSON.parse(content); } catch { alert('Invalid JSON'); return; }
    if (data.type !== expectedType) { alert(`Expected "${expectedType}" but got "${data.type}"`); return; }

    try {
        const result = await rooms.postToRoom(prefix, data, expectedType);
        if (typeof result === 'number') {
            alert(`Sent ${result} encrypted shares`);
        }
        textarea.value = '';
    } catch (e) {
        alert('Failed to publish: ' + e.message);
    }
};

// Enable E2E encryption
window.enableDkgEncryption = () => {
    const secretInput = document.getElementById('dkg-secret-coefficient');
    const secret = secretInput.value.trim();

    if (!secret) { alert('Enter your secret coefficient'); return; }
    if (!encryption.isValidHex64(secret)) { alert('Must be 64 hex characters'); return; }

    rooms.rooms.dkg.myIndex = parseInt(document.getElementById('dkg-my-index').value);
    rooms.setSecretCoefficient('dkg', secret);

    ui.updateEncryptionStatus('dkg', true);
    secretInput.disabled = true;
    secretInput.style.background = 'rgba(46, 204, 113, 0.1)';
    ui.refreshDkgUI('dkg');

    alert('E2E encryption enabled!');
};

// Signing Room Actions
window.joinSigningRoom = () => {
    const roomId = document.getElementById('sign-room-id').value.trim();
    const myIndex = parseInt(document.getElementById('sign-my-index').value);

    if (!roomId) { alert('Enter a Room ID'); return; }

    document.getElementById('sign-status-text').textContent = 'Connecting...';
    rooms.subscribeSigning(roomId, myIndex, (event) => {
        if (event === 'connected') {
            document.getElementById('sign-status-indicator').classList.add('connected');
            document.getElementById('sign-status-text').textContent = 'Connected';
            document.getElementById('sign-status-text').style.color = '#00d4ff';
        }
        ui.refreshSigningUI();
    });
};

window.setupSigningEncryption = () => {
    const keysInput = document.getElementById('sign-group-keys').value.trim();
    const secretInput = document.getElementById('sign-my-secret').value.trim();

    if (!keysInput || !secretInput) { alert('Enter group keys and secret'); return; }
    if (!encryption.isValidHex64(secretInput)) { alert('Secret must be 64 hex characters'); return; }

    const count = rooms.setupSigningKeys(secretInput, keysInput);
    if (count < 2) { alert('Need at least 2 group members'); return; }

    ui.updateSigningEncryptionStatus(count);
    alert(`Private mode enabled with ${count} group members`);
};

window.broadcastEncryptedNonce = async () => {
    const input = document.getElementById('sign-nonce-input').value.trim();
    if (!input) { alert('Paste your nonce JSON'); return; }

    let data;
    try { data = JSON.parse(input); } catch { alert('Invalid JSON'); return; }

    try {
        const sent = await rooms.broadcastToSigningGroup('signing_nonce_encrypted', data);
        document.getElementById('sign-nonce-input').value = '';
        alert(`Sent encrypted nonce to ${sent} members`);
    } catch (e) {
        alert('Failed: ' + e.message);
    }
};

window.broadcastEncryptedShare = async () => {
    const input = document.getElementById('sign-share-input').value.trim();
    if (!input) { alert('Paste your signature share JSON'); return; }

    let data;
    try { data = JSON.parse(input); } catch { alert('Invalid JSON'); return; }

    try {
        const sent = await rooms.broadcastToSigningGroup('signing_share_encrypted', data);
        document.getElementById('sign-share-input').value = '';
        alert(`Sent encrypted share to ${sent} members`);
    } catch (e) {
        alert('Failed: ' + e.message);
    }
};

window.copyCollectedNonces = () => {
    const nonces = rooms.rooms.signing.nonces;
    if (nonces.size === 0) { alert('No nonces collected'); return; }

    const json = Array.from(nonces.values()).map(n => JSON.stringify(n)).join(' ');
    navigator.clipboard.writeText(json);
    alert(`Copied ${nonces.size} nonces`);
};

window.copyCollectedShares = () => {
    const shares = rooms.rooms.signing.shares;
    if (shares.size === 0) { alert('No shares collected'); return; }

    const json = Array.from(shares.values()).map(s => JSON.stringify(s)).join(' ');
    navigator.clipboard.writeText(json);
    alert(`Copied ${shares.size} shares`);
};

// Utility functions
window.copyToClipboard = ui.copyToClipboard;
window.switchMainTab = ui.switchMainTab;
