import { api } from './api.js';

export const store = {
    settings: null,
    instances: [],
    running: [],
    accounts: [],
    versions: null
};

const listeners = new Map();

export function on(key, handler) {
    if (!listeners.has(key)) listeners.set(key, new Set());
    listeners.get(key).add(handler);
}

export function emit(key) {
    for (const handler of listeners.get(key) ?? []) handler();
}

export async function loadSettings() {
    store.settings = await api.get_settings();
    return store.settings;
}

export async function saveSettings(patch = {}) {
    store.settings = { ...store.settings, ...patch };
    await api.save_settings({ settings: store.settings });
    emit('settings');
}

export async function loadInstances() {
    const [instances, running] = await api.list_instances();
    store.instances = instances;
    store.running = running;
    emit('instances');
}

export async function loadAccounts() {
    store.accounts = await api.list_accounts();
    emit('accounts');
}

export const idle = () => document.hidden || store.running.length > 0;

export function selectedInstance() {
    const id = store.settings?.selectedInstance;
    return store.instances.find((i) => i.id === id) ?? store.instances[0] ?? null;
}

export function selectedAccount() {
    const id = store.settings?.selectedAccount;
    return store.accounts.find((a) => a.id === id) ?? store.accounts[0] ?? null;
}

export const isRunning = (id) => store.running.includes(id);

const faces = new Map();
const faceTried = new Set();

function storedFace(id) {
    try {
        return localStorage.getItem(`face:${id}`);
    } catch {
        return null;
    }
}

function loadImage(url) {
    return new Promise((resolve, reject) => {
        const img = new Image();
        img.crossOrigin = 'anonymous';
        img.onload = () => resolve(img);
        img.onerror = reject;
        img.src = url;
    });
}

export function faceFromSkin(img) {
    const canvas = document.createElement('canvas');
    canvas.width = canvas.height = 64;
    const ctx = canvas.getContext('2d');
    ctx.imageSmoothingEnabled = false;
    ctx.drawImage(img, 8, 8, 8, 8, 0, 0, 64, 64);
    ctx.drawImage(img, 40, 8, 8, 8, 0, 0, 64, 64);
    return canvas.toDataURL('image/png');
}

async function loadFace(account) {
    faceTried.add(account.id);
    try {
        const profile = await api.account_profile({ id: account.id });
        const skin = (profile.skins || []).find((s) => s.state === 'ACTIVE');
        if (!skin?.url) return;
        const face = faceFromSkin(await loadImage(skin.url.replace(/^http:/, 'https:')));
        faces.set(account.id, face);
        try {
            localStorage.setItem(`face:${account.id}`, face);
        } catch {}
        for (const img of document.querySelectorAll(`img[data-avatar="${account.id}"]`)) img.src = face;
    } catch {}
}

export function refreshAvatar(account) {
    if (account?.kind === 'msa') loadFace(account);
}

export function avatarUrl(account) {
    if (!account || account.kind !== 'msa') return 'assets/icon.png';
    if (!faceTried.has(account.id)) loadFace(account);
    return faces.get(account.id) ?? storedFace(account.id) ?? `https://api.mineatar.io/face/${account.uuid.replace(/-/g, '')}?scale=8`;
}
