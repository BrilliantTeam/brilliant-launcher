import { api, fileSrc } from './api.js';
import { toast } from './ui.js';
import { refreshSessions } from './views/accounts.js';
import { emit, on, saveSettings, selectedInstance, store } from './store.js';

const PREFETCH_DELAY_MS = 12_000;
const PREFETCH_EVERY_MS = 2 * 60 * 60 * 1000;
const NEWS_EVERY_MS = 10 * 60 * 1000;
const BOARDS = ['general', 'event', 'update'];

export const ready = new Set();
let prefetchTimer = 0;
let prefetching = false;

async function prefetch() {
    const instance = selectedInstance();
    if (!instance || prefetching || store.running.length || document.body.classList.contains('launching')) return;
    prefetching = true;
    try {
        await api.prefetch({ id: instance.id });
        ready.add(instance.id);
        emit('ready');
    } catch {
        ready.delete(instance.id);
    } finally {
        prefetching = false;
    }
}

function schedulePrefetch(delay = PREFETCH_DELAY_MS) {
    clearTimeout(prefetchTimer);
    prefetchTimer = setTimeout(prefetch, delay);
}

async function notifyOnce(key, title, body) {
    if (!store.settings?.notifications || store.settings.notified?.includes(key)) return;
    await api.notify_user({ title, body }).catch(() => {});
    await saveSettings({ notified: [...(store.settings.notified || []), key].slice(-30) }).catch(() => {});
}

async function checkNews() {
    const pages = await Promise.all(BOARDS.map((ch) => api.site_json({ path: `/api/announcements/plain?ch=${ch}` }).catch(() => null)));
    const newest = pages.flatMap((page) => page?.messages || []).sort((a, b) => b.time - a.time)[0];
    if (!newest) return;
    const seen = Number(store.settings?.lastAnnouncement || 0);
    if (seen && newest.time > seen && store.settings?.notifications) {
        const { titleOf } = await import('./views/news.js');
        await api.notify_user({ title: '輝煌有新公告', body: titleOf(newest) }).catch(() => {});
    }
    if (newest.time > seen) await saveSettings({ lastAnnouncement: String(newest.time) }).catch(() => {});
}

export function upgradeTarget(instance) {
    const recommended = store.versions?.recommended;
    if (!instance || !recommended || instance.loader !== 'fabric' || instance.mcVersion === recommended) return null;
    const [a, b] = [recommended, instance.mcVersion].map((v) => v.split('.').map((n) => parseInt(n, 10) || 0));
    for (let i = 0; i < 3; i++) if ((a[i] || 0) !== (b[i] || 0)) return (a[i] || 0) > (b[i] || 0) ? recommended : null;
    return null;
}

async function checkUpgrade() {
    const target = upgradeTarget(selectedInstance());
    if (target) await notifyOnce(`upgrade:${target}`, `輝煌已更新到 Minecraft ${target}`, '打開啟動器，一鍵把你的實例升級到新版本。');
}

async function tuneHardware() {
    if (store.settings?.hardwareTuned) return;
    const info = await api.system_info().catch(() => null);
    if (!info?.memoryMb) return;
    await saveSettings({ memoryMb: info.recommendedMb, hardwareTuned: true });
    toast(`已依你的電腦（${Math.round(info.memoryMb / 1024)} GB 記憶體）把遊戲記憶體設為 ${info.recommendedMb} MB，可在設定中調整。`);
}

export function gpuName() {
    try {
        const gl = document.createElement('canvas').getContext('webgl');
        const ext = gl?.getExtension('WEBGL_debug_renderer_info');
        const raw = ext ? String(gl.getParameter(ext.UNMASKED_RENDERER_WEBGL)) : '';
        const angle = /^ANGLE \([^,]+,\s*(.+?)(?:\s*\(0x[0-9a-f]+\))?\s*(?:Direct3D|OpenGL|Vulkan)/i.exec(raw);
        return (angle ? angle[1] : raw).trim();
    } catch {
        return '';
    }
}

export function isIntegratedGpu(name = gpuName()) {
    return /intel|uhd|iris|vega \d|radeon\(tm\) graphics|radeon graphics|microsoft basic/i.test(name) && !/arc a\d|nvidia|geforce|rtx|gtx|radeon rx/i.test(name);
}

const music = new Audio();
music.loop = true;
let musicSrc = '';
let waitingGesture = false;

function applyMusic() {
    const path = store.settings?.music || '';
    const volume = (store.settings?.musicVolume ?? 40) / 100;
    if (path !== musicSrc) {
        musicSrc = path;
        if (path) music.src = fileSrc(path);
        else music.removeAttribute('src');
    }
    music.volume = Math.min(1, Math.max(0, volume));
    const shouldPlay = path && volume > 0 && !document.hidden && !store.running.length && !document.body.classList.contains('launching');
    if (!shouldPlay) return music.pause();
    music.play().catch(() => {
        if (waitingGesture) return;
        waitingGesture = true;
        document.addEventListener('pointerdown', () => { waitingGesture = false; applyMusic(); }, { once: true });
    });
}

export function startServices() {
    refreshSessions();
    setInterval(refreshSessions, 30 * 60 * 1000);
    applyMusic();
    on('settings', applyMusic);
    on('instances', applyMusic);
    document.addEventListener('visibilitychange', applyMusic);
    new MutationObserver(applyMusic).observe(document.body, { attributes: true, attributeFilter: ['class'] });
    tuneHardware().catch(() => {});
    schedulePrefetch();
    setInterval(() => schedulePrefetch(0), PREFETCH_EVERY_MS);
    let lastSelected = selectedInstance()?.id;
    on('instances', () => {
        const id = selectedInstance()?.id;
        if (id !== lastSelected) {
            lastSelected = id;
            schedulePrefetch(4000);
        }
    });
    on('settings', () => {
        const id = selectedInstance()?.id;
        if (id !== lastSelected) {
            lastSelected = id;
            schedulePrefetch(4000);
        }
    });
    setTimeout(() => checkNews().catch(() => {}), 20_000);
    setInterval(() => checkNews().catch(() => {}), NEWS_EVERY_MS);
    setTimeout(() => checkUpgrade().catch(() => {}), 8000);
}

export { notifyOnce };
