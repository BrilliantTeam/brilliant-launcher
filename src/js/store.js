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

export function selectedInstance() {
    const id = store.settings?.selectedInstance;
    return store.instances.find((i) => i.id === id) ?? store.instances[0] ?? null;
}

export function selectedAccount() {
    const id = store.settings?.selectedAccount;
    return store.accounts.find((a) => a.id === id) ?? store.accounts[0] ?? null;
}

export const isRunning = (id) => store.running.includes(id);

export function avatarUrl(account) {
    if (!account || account.kind !== 'msa') return 'assets/icon.png';
    return `https://api.mineatar.io/face/${account.uuid.replace(/-/g, '')}?scale=8`;
}
