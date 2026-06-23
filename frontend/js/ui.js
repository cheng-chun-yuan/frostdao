// UI update utilities
import { rooms } from './rooms.js';

// Copy text to clipboard with visual feedback
export function copyToClipboard(elementId) {
    const el = document.getElementById(elementId);
    navigator.clipboard.writeText(el.textContent);
    el.style.background = '#1a3a1a';
    setTimeout(() => el.style.background = '', 200);
}

// Update connection status
export function updateConnectionStatus(prefix, connected) {
    const indicator = document.getElementById(`${prefix}-status-indicator`);
    const text = document.getElementById(`${prefix}-status-text`);

    if (connected) {
        indicator?.classList.add('connected');
        if (text) {
            text.textContent = 'Connected';
            text.style.color = '#00d4ff';
        }
    } else {
        indicator?.classList.remove('connected');
        if (text) {
            text.textContent = 'Disconnected';
            text.style.color = '#888';
        }
    }
}

// Refresh DKG/HTSS room UI
export function refreshDkgUI(prefix) {
    const state = rooms[prefix];
    const nParties = parseInt(document.getElementById(`${prefix}-n-parties`)?.value || '3');

    ['round1', 'round2'].forEach(round => {
        const key = round === 'round1' ? 'keygen_round1' : 'keygen_round2';
        const data = state.data[key];

        updateCount(`${prefix}-${round}-count`, data.size, nParties);
        updateEntries(`${prefix}-${round}-entries`, data);
        updateCopySection(`${prefix}-${round}`, data, nParties);
    });

    updateEncryptionKeyCount(prefix);
}

function updateCount(elementId, current, total) {
    const el = document.getElementById(elementId);
    if (el) el.textContent = `${current} / ${total}`;
}

function updateEntries(elementId, dataMap) {
    const el = document.getElementById(elementId);
    if (!el) return;

    if (dataMap.size === 0) {
        el.innerHTML = '<div class="empty-state">Waiting...</div>';
        return;
    }

    el.innerHTML = '';
    Array.from(dataMap.values())
        .sort((a, b) => a.party_index - b.party_index)
        .forEach(entry => {
            const div = document.createElement('div');
            div.className = 'entry';
            const rankInfo = entry.rank !== undefined ? ` (r${entry.rank})` : '';
            div.innerHTML = `
                <div class="entry-header">
                    <span class="entry-party">Party ${entry.party_index}${rankInfo}</span>
                </div>
                <div class="entry-data">${JSON.stringify(entry, null, 2).slice(0, 200)}...</div>
            `;
            el.appendChild(div);
        });
}

function updateCopySection(prefix, dataMap, nParties) {
    const copySection = document.getElementById(`${prefix}-copy-section`);
    const output = document.getElementById(`${prefix}-output`);

    if (!copySection || !output) return;

    if (dataMap.size >= nParties) {
        copySection.style.display = 'block';
        const jsonObjs = Array.from(dataMap.values())
            .sort((a, b) => a.party_index - b.party_index)
            .map(e => {
                const c = { ...e };
                delete c.created_at;
                return JSON.stringify(c);
            });
        output.textContent = jsonObjs.join(' ');
    } else {
        copySection.style.display = 'none';
    }
}

function updateEncryptionKeyCount(prefix) {
    const el = document.getElementById(`${prefix}-encryption-keys-count`);
    if (el) {
        const count = rooms[prefix].data.encryptionKeys.size;
        el.textContent = `${count} party encryption key(s) stored`;
    }
}

// Update encryption status
export function updateEncryptionStatus(prefix, enabled) {
    const indicator = document.getElementById(`${prefix}-encryption-indicator`);
    const status = document.getElementById(`${prefix}-encryption-status`);

    if (enabled) {
        indicator?.classList.add('connected');
        if (status) {
            status.textContent = 'Enabled';
            status.style.color = '#2ecc71';
        }
    }
}

// Refresh signing room UI
export function refreshSigningUI() {
    const state = rooms.signing;

    document.getElementById('sign-nonce-count').textContent = `${state.nonces.size} received`;
    document.getElementById('sign-share-count').textContent = `${state.shares.size} received`;

    updateSigningEntries('sign-received-nonces', state.nonces, 'nonces');
    updateSigningEntries('sign-received-shares', state.shares, 'shares');
}

function updateSigningEntries(elementId, dataMap, type) {
    const el = document.getElementById(elementId);
    if (!el) return;

    if (dataMap.size === 0) {
        el.textContent = `No ${type} received yet.`;
        return;
    }

    el.innerHTML = Array.from(dataMap.entries())
        .map(([idx]) => `<div class="entry"><span class="entry-party">Party ${idx}</span></div>`)
        .join('');
}

// Update signing encryption status
export function updateSigningEncryptionStatus(count) {
    const status = document.getElementById('sign-encryption-status');
    if (status) {
        status.textContent = `${count} keys configured`;
        status.style.color = '#2ecc71';
    }
}

// Tab switching
export function switchMainTab(tabName) {
    document.querySelectorAll('.main-tab').forEach(t => t.classList.remove('active'));
    document.querySelectorAll('.main-tab-content').forEach(t => t.classList.remove('active'));

    const tabs = ['dkg', 'htss', 'sign', 'hd', 'reshare', 'backup', 'verify'];
    const idx = tabs.indexOf(tabName);
    document.querySelectorAll('.main-tab')[idx]?.classList.add('active');
    document.getElementById(`${tabName}-content`)?.classList.add('active');
}
