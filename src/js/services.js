import { api, fileSrc } from './api.js';
import { dialog, h, toast } from './ui.js';
import { refreshSessions } from './views/accounts.js';
import { emit, on, saveSettings, selectedInstance, store } from './store.js';

const PREFETCH_DELAY_MS = 12_000;
const PREFETCH_EVERY_MS = 14 * 60 * 1000;
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
    if (!instance || instance.general || !recommended || instance.loader !== 'fabric' || instance.mcVersion === recommended) return null;
    const [a, b] = [recommended, instance.mcVersion].map((v) => v.split('.').map((n) => parseInt(n, 10) || 0));
    for (let i = 0; i < 3; i++) if ((a[i] || 0) !== (b[i] || 0)) return (a[i] || 0) > (b[i] || 0) ? recommended : null;
    return null;
}

const upgradeReady = new Set();

async function checkUpgrade() {
    const info = await api.launcher_info().catch(() => null);
    if (info?.recommended && store.versions && store.versions.recommended !== info.recommended) {
        store.versions.recommended = info.recommended;
        store.versions.modVersions = info.versions;
        emit('instances');
    }
    const instance = selectedInstance();
    const target = upgradeTarget(instance);
    if (!target) return;
    const key = `${instance.id}:${target}`;
    if (!upgradeReady.has(key) && !store.running.length && store.online !== false) {
        upgradeReady.add(key);
        api.prefetch_upgrade({ id: instance.id, mc: target }).catch(() => upgradeReady.delete(key));
    }
    await notifyOnce(`upgrade:${target}`, `輝煌已更新到 Minecraft ${target}`, '打開啟動器，一鍵把你的實例升級到新版本。');
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

function setOnline(value) {
    if (store.online === value) return;
    const first = store.online === undefined;
    store.online = value;
    document.getElementById('net-chip').hidden = value;
    emit('network');
    if (first && value) return;
    toast(value ? '網路已恢復連線，正在更新伺服器與公告資訊' : '目前沒有網路連線：已下載過的實例仍可開始單人遊戲，登入、下載與更新會在恢復後自動繼續', value ? '' : 'warn');
}

let probing = null;

function probeNetwork() {
    probing ??= api.net_status()
        .catch(() => navigator.onLine)
        .then((up) => setOnline(up !== false))
        .finally(() => { probing = null; });
    return probing;
}

export function startServices() {
    addEventListener('offline', () => setOnline(false));
    addEventListener('online', () => probeNetwork());
    addEventListener('brilliant:net-error', () => probeNetwork());
    document.getElementById('net-chip').addEventListener('click', async () => {
        const list = (items) => h('ul', {}, ...items.map((t) => h('li', {}, t)));
        const go = await dialog({
            title: '離線模式',
            content: h('div', {},
                h('p', {}, '啟動器目前連不上網路。'),
                h('b', {}, '還能做的事'),
                list(['開始已經完整下載過的實例，玩單人遊戲', '管理實例、存檔、截圖與相簿', '調整設定、啟用或停用已安裝的模組']),
                h('b', {}, '暫停中，網路恢復後自動繼續'),
                list(['微軟帳號登入與權杖續期（連線伺服器需要）', '下載遊戲檔案、模組與更新', '分流人數、公告、活動與我的輝煌']),
                h('p', { class: 'hint' }, '請確認網路線、Wi-Fi 或代理伺服器設定。啟動器每 15 秒會自動檢查一次，恢復後這個標籤就會消失。')),
            buttons: [{ label: '關閉', value: null }, { label: '重新檢查連線', value: 'retry' }]
        });
        if (go !== 'retry') return;
        await probeNetwork();
        if (store.online === false) toast('還是連不上網路，請檢查網路設定後再試', 'warn');
    });
    document.addEventListener('visibilitychange', () => { if (!document.hidden) probeNetwork(); });
    probeNetwork();
    setInterval(() => { if (!document.hidden || store.online === false) probeNetwork(); }, 15_000);
    on('network', () => {
        if (store.online === false) return;
        schedulePrefetch(3000);
        checkNews().catch(() => {});
        checkUpgrade().catch(() => {});
    });
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
    setInterval(() => checkUpgrade().catch(() => {}), 30 * 60 * 1000);
}

export { notifyOnce };
