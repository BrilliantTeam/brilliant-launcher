import { api, appVersion, appWindow, listen, openUrl, setZoom } from './api.js';
import { icon } from './icons.js';
import { $, $$, dialog, fail, h, rangeFill, toast, guard } from './ui.js';
import { emit, loadAccounts, loadInstances, loadSettings, on, saveSettings, selectedAccount, store } from './store.js';
import { initHome } from './views/home.js';
import { initInstances } from './views/instances.js';
import { initMods } from './views/mods.js';
import { initAccounts } from './views/accounts.js';
import { initSettings } from './views/settings.js';
import { initProfile } from './views/profile.js';
import { initAlbum } from './views/album.js';
import { initExplore } from './views/explore.js';
import { notifyOnce, startServices } from './services.js';
import { initLog } from './views/logview.js';
import { initReports } from './report.js';

const TITLES = {
    profile: '我的輝煌',
    album: '相簿與回顧',
    home: '首頁',
    instances: '實例管理',
    mods: '模組與資源',
    explore: '探索',
    accounts: '帳號',
    settings: '設定',
    log: '遊戲日誌',
    about: '關於啟動器'
};

function show(view) {
    const name = TITLES[view] ? view : 'home';
    document.body.dataset.view = name;
    document.title = name === 'home' ? '輝煌啟動器' : `${TITLES[name]}｜輝煌啟動器`;
    $$('.view').forEach((el) => el.classList.toggle('active', el.id === `view-${name}`));
    const active = $(`#view-${name}`);
    if (active) {
        active.classList.add('entering');
        clearTimeout(active.enterTimer);
        active.enterTimer = setTimeout(() => active.classList.remove('entering'), 700);
    }
    $$('.rail a').forEach((el) => el.classList.toggle('active', el.dataset.view === (name === 'mods' ? 'instances' : name)));
    $('.page-wrap').scrollTop = 0;
}

const THEMES = {
    auto: { icon: 'invert', label: '主題：跟隨系統（點一下切換為亮色）' },
    light: { icon: 'sun', label: '主題：亮色（點一下切換為深色）' },
    dark: { icon: 'moon', label: '主題：深色（點一下改為跟隨系統）' }
};

let appliedScale = 1;

let walkersOn = false;
let walkersLoading = false;

async function applyWalkers() {
    const want = Boolean(store.settings?.walkers);
    const toggle = $('#walker-toggle');
    const label = want ? '關閉虛擬夥伴' : '開啟虛擬夥伴';
    toggle.setAttribute('aria-pressed', String(want));
    toggle.title = label;
    toggle.setAttribute('aria-label', label);
    if (want === walkersOn || walkersLoading) return;
    walkersOn = want;
    if (window.toggleWalkers) return window.toggleWalkers();
    if (!want) return;
    walkersLoading = true;
    try { localStorage.setItem('walkers', 'on'); } catch {}
    window.walkerFetch = (url, options) => fetch(url, { ...options, cache: 'reload', credentials: 'omit' });
    window.walkerMe = () => Promise.resolve({ ok: true, json: () => (selectedAccount()?.name ? { name: selectedAccount().name } : {}) });
    const online = await api.site_json({ path: '/api/online' }).catch(() => ({}));
    document.head.append(h('link', { rel: 'stylesheet', href: 'vendor/walkers.css' }));
    $('.stage').append(
        h('div', { class: 'walkers', id: 'walkers', 'data-players': JSON.stringify(online.players || []), 'data-guilds': JSON.stringify(online.guilds || {}), 'data-info': JSON.stringify(online.info || {}), 'data-frames': '37', 'aria-hidden': 'true' }),
        h('div', { class: 'walker-layer', id: 'walker-layer', 'aria-hidden': 'true' }));
    const load = (src, attrs = {}) => new Promise((done) => document.body.append(h('script', { src, onload: done, onerror: done, ...attrs })));
    await load('vendor/gramophone.js', { 'data-base': 'https://www.brilliantw.net/gramophone/sound/' });
    await load('vendor/walkers.js');
    walkersLoading = false;
    if (!store.settings?.walkers) return applyWalkers();
    setInterval(() => {
        if (!walkersOn || store.online === false) return;
        api.site_json({ path: '/api/online' }).then((d) => window.syncWalkers?.(d.players || [], d.guilds || {}, d.info || {})).catch(() => {});
    }, 60_000);
}

function applyTheme() {
    const root = document.documentElement;
    const theme = THEMES[store.settings?.theme] ? store.settings.theme : 'auto';
    if (theme === 'auto') delete root.dataset.theme;
    else root.dataset.theme = theme;
    root.dataset.font = store.settings?.font ?? 'pixel';
    const toggle = $('#theme-toggle');
    toggle.replaceChildren(icon(THEMES[theme].icon));
    toggle.title = THEMES[theme].label;
    toggle.setAttribute('aria-label', THEMES[theme].label);
    const accent = /^#[0-9a-f]{6}$/i.test(store.settings?.accent ?? '') ? store.settings.accent : '';
    if (accent) {
        const [r, g, b] = [1, 3, 5].map((i) => parseInt(accent.slice(i, i + 2), 16));
        root.style.setProperty('--accent', accent);
        root.style.setProperty('--accent-hover', `color-mix(in srgb, ${accent} 85%, #000)`);
        root.style.setProperty('--accent-ink', 0.299 * r + 0.587 * g + 0.114 * b > 140 ? '#111111' : '#ffffff');
    } else {
        for (const key of ['--accent', '--accent-hover', '--accent-ink']) root.style.removeProperty(key);
    }
    root.dataset.motion = store.settings?.reduceMotion ? 'off' : 'on';
    const scale = (store.settings?.uiScale || 100) / 100;
    if (scale !== appliedScale) {
        appliedScale = scale;
        setZoom(scale).catch(() => {});
    }
    const font = $('#font-toggle');
    font.title = root.dataset.font === 'system' ? '字型：系統字型（點一下改為點陣字型）' : '字型：點陣字型（點一下改為系統字型）';
}

function mountIcons() {
    for (const el of document.querySelectorAll('i[data-icon]')) {
        el.replaceWith(icon(el.dataset.icon, ['ico', el.className].filter(Boolean).join(' ')));
    }
}

function initChrome() {
    const editable = 'input, textarea, select, .console, .crash-exception';
    document.addEventListener('contextmenu', (e) => {
        if (!e.target.closest?.(editable)) e.preventDefault();
    });
    document.addEventListener('dragstart', (e) => {
        if (!e.target.closest?.(editable)) e.preventDefault();
    });
    document.addEventListener('input', (e) => {
        if (e.target.matches?.('input[type="range"]')) rangeFill(e.target);
    });
    mountIcons();
    const win = appWindow();
    $('#win-min').addEventListener('click', () => win.minimize());
    $('#win-max').addEventListener('click', () => win.toggleMaximize());
    $('#win-close').addEventListener('click', () => (store.settings?.closeToTray ? win.hide() : win.close()));
    const dragFromDialog = (e) => {
        const dlg = e.target.closest?.('dialog[open]');
        if (!dlg || e.button !== 0) return false;
        const box = dlg.getBoundingClientRect();
        const outside = e.target === dlg && (e.clientX < box.left || e.clientX > box.right || e.clientY < box.top || e.clientY > box.bottom);
        return outside || e.target.matches('dialog h2, dialog .dlg-body > h2');
    };
    document.addEventListener('mousedown', (e) => {
        if (!dragFromDialog(e)) return;
        e.preventDefault();
        if (e.detail === 2) win.toggleMaximize();
        else win.startDragging?.();
    });

    const views = $$('.rail a[data-view]').map((a) => a.dataset.view);
    const mac = navigator.userAgent.includes('Mac');
    if (mac) $$('kbd.mod-key').forEach((k) => { k.textContent = '⌘'; });
    document.addEventListener('keydown', (e) => {
        if (e.isComposing || e.keyCode === 229 || e.altKey || (mac ? !e.metaKey || e.ctrlKey : !e.ctrlKey || e.metaKey)) return;
        const digit = /^Digit([1-9])$/.exec(e.code)?.[1];
        if (e.code === 'Enter' || e.code === 'NumpadEnter') {
            e.preventDefault();
            location.hash = '#home';
            emit('launch');
        } else if (digit && views[digit - 1]) {
            e.preventDefault();
            location.hash = `#${views[digit - 1]}`;
        } else if (e.code === 'Comma') {
            e.preventDefault();
            location.hash = '#settings';
        } else if (e.code === 'KeyF') {
            e.preventDefault();
            if (location.hash === '#settings') {
                $('#settings-search').focus();
                $('#settings-search').select();
                return;
            }
            location.hash = '#mods';
            setTimeout(() => $('#mod-search').focus(), 50);
        } else if (e.code === 'KeyR' || e.code === 'KeyP') {
            e.preventDefault();
        }
    });

    $('#theme-toggle').addEventListener('click', async (e) => {
        const order = ['auto', 'light', 'dark'];
        const current = order.indexOf(store.settings.theme ?? 'auto');
        store.settings.theme = order[(current + 1) % order.length];
        const root = document.documentElement;
        root.style.setProperty('--reveal-x', `${e.clientX}px`);
        root.style.setProperty('--reveal-y', `${e.clientY}px`);
        if (document.startViewTransition && !store.settings.reduceMotion && !matchMedia('(prefers-reduced-motion: reduce)').matches) document.startViewTransition(applyTheme);
        else applyTheme();
        await api.save_settings({ settings: store.settings }).catch(fail);
    });

    $('#walker-toggle').addEventListener('click', () => saveSettings({ walkers: !store.settings?.walkers }).catch(fail));

    $('#font-toggle').addEventListener('click', async () => {
        store.settings.font = store.settings.font === 'system' ? 'pixel' : 'system';
        applyTheme();
        await api.save_settings({ settings: store.settings }).catch(fail);
    });

    document.addEventListener('click', (e) => {
        const link = e.target.closest('a[data-url]');
        if (!link) return;
        e.preventDefault();
        openUrl(link.dataset.url);
    });

    addEventListener('hashchange', () => show(location.hash.slice(1)));
}

const RANK = { alpha: 0, beta: 1 };

export function versionKey(version) {
    const [core, pre = ''] = String(version).split('-');
    const m = /^(alpha|beta)\.(\d+)$/.exec(pre);
    return [...core.split('.').map(Number).concat([0, 0, 0]).slice(0, 3), m ? RANK[m[1]] : 2, m ? Number(m[2]) : 0];
}

export const channelOfVersion = (version) => (/-(alpha|beta)\./.exec(String(version)) || [])[1] || (newerVersion('1.1.0', version) ? 'alpha' : 'stable');

export function newerVersion(latest, current) {
    const a = versionKey(latest);
    const b = versionKey(current);
    for (let i = 0; i < a.length; i++) {
        if ((a[i] || 0) !== (b[i] || 0)) return (a[i] || 0) > (b[i] || 0);
    }
    return false;
}

let pendingUpdate = null;
const updater = { version: '', path: null, downloading: false };

const UPDATE_ASSET = { windows: 'windows', mac: 'macUpdate', linux: 'linux' };
let updateKind = null;
const updateAsset = (latest) => (updateKind ? latest.platforms?.[UPDATE_ASSET[updateKind]] ?? (updateKind === 'windows' ? latest : null) : null);
const canSelfUpdate = (latest) => !!updateAsset(latest)?.sha256 && !!updateAsset(latest)?.signature;

function updateChip() {
    const latest = pendingUpdate?.latest;
    if (!latest) return;
    const chip = $('#update-chip');
    chip.hidden = false;
    chip.classList.toggle('ready', !!updater.path);
    if (!updater.downloading) $('#update-label').textContent = updater.path ? `重新啟動以更新 v${latest.version}` : `更新 v${latest.version}`;
}

async function downloadUpdate() {
    const latest = pendingUpdate?.latest;
    if (!latest || !canSelfUpdate(latest) || updater.downloading || (updater.path && updater.version === latest.version)) return;
    updater.downloading = true;
    updater.version = latest.version;
    updater.path = null;
    $('#update-label').textContent = '下載更新 0%';
    try {
        const asset = updateAsset(latest);
        updater.path = await api.download_update({ url: asset.url, file: asset.file, sha256: asset.sha256, signature: asset.signature });
        notifyOnce(`launcher-ready:${latest.version}`, `輝煌啟動器 v${latest.version} 已準備好`, '點右上角「重新啟動以更新」，幾秒內就會完成並重新開啟。');
    } catch (error) {
        updater.path = null;
        if (/簽章/.test(String(error?.message ?? error))) fail(error);
    } finally {
        updater.downloading = false;
        updateChip();
    }
}

async function applyUpdate() {
    if (store.running.length) return toast('遊戲還在執行中，關閉遊戲後再更新', 'warn');
    toast('正在更新，啟動器會自動重新開啟⋯');
    await api.apply_update({ path: updater.path }).catch((error) => {
        updater.path = null;
        updateChip();
        fail(error);
    });
}

async function checkUpdate(manual = false) {
    const mode = store.settings?.launcherUpdate || 'auto';
    if (mode === 'off' && !manual) return;
    const [info, current, kind] = await Promise.all([api.check_update().catch(() => null), appVersion().catch(() => ''), api.update_platform().catch(() => '')]);
    updateKind = kind;
    const latest = info?.launcher;
    store.update = { checkedAt: Date.now(), current, latest: latest ?? null, reachable: !!info, newer: !!(latest && current && newerVersion(latest.version, current)), signed: !!updateAsset(latest ?? {})?.signature, self: !!updateKind };
    emit('update-status');
    if (!latest || !current || !newerVersion(latest.version, current)) return;
    const first = !pendingUpdate;
    pendingUpdate = { latest, current };
    updateChip();
    if (canSelfUpdate(latest) && mode === 'auto') downloadUpdate();
    else notifyOnce(`launcher:${latest.version}`, `輝煌啟動器 v${latest.version} 已推出`, '打開啟動器，點右上角的更新按鈕前往下載。');
    if (!first) return;
    listen('launcher://update-progress', ({ done, total }) => {
        if (updater.downloading && total) $('#update-label').textContent = `下載更新 ${Math.floor((done * 100) / total)}%`;
    });
    $('#update-chip').addEventListener('click', guard(async () => {
        const { latest, current } = pendingUpdate;
        const self = canSelfUpdate(latest);
        const ready = !!updater.path;
        const status = !self ? '這個平台需要到官網下載新版。'
            : ready ? '新版已下載完成。按「重新啟動並更新」，啟動器會在背景完成安裝並自動重新開啟，不會出現安裝精靈，設定與遊戲資料都會保留。'
            : updater.downloading ? '正在背景下載新版，完成後右上角會變成「重新啟動以更新」。'
            : updater.version === latest.version ? '下載失敗了，可以再試一次，或到官網手動下載。'
            : '按「下載更新」會在背景下載，並先驗證輝煌官方數位簽章，完成後再由你決定何時重新啟動。';
        const buttons = !self ? [{ label: '稍後', value: null }, { label: '前往官網下載', value: 'web' }]
            : ready ? [{ label: '稍後', value: null }, { label: '重新啟動並更新', value: 'apply' }]
            : updater.downloading ? [{ label: '關閉', value: null }]
            : [{ label: '前往官網', value: 'web' }, { label: updater.version === latest.version ? '重新下載' : '下載更新', value: 'retry' }];
        const go = await dialog({
            title: `輝煌啟動器 v${latest.version}`,
            content: h('div', {},
                h('p', { class: 'hint' }, `目前版本 v${current}`),
                h('p', { style: 'white-space:pre-wrap' }, latest.notes || '有新版本可以更新。'),
                h('p', { class: 'hint' }, status)),
            buttons
        });
        if (go === 'apply') await applyUpdate();
        else if (go === 'retry') downloadUpdate();
        else if (go === 'web') openUrl(latest.page);
    }));
}

async function boot() {
    try {
        await loadSettings();
    } catch (error) {
        fail(error);
        store.settings = {};
    }
    applyTheme();
    initChrome();
    on('settings', applyTheme);
    applyWalkers();
    on('settings', applyWalkers);

    await Promise.all([loadInstances().catch(fail), loadAccounts().catch(fail)]);
    window.addEventListener('focus', () => loadAccounts().catch(() => {}));

    initHome();
    initInstances();
    initMods();
    initAccounts();
    initSettings();
    initProfile();
    initAlbum();
    initExplore();
    initLog();
    initReports();

    emit('settings');
    show(location.hash.slice(1) || 'home');
    checkUpdate();
    setInterval(() => checkUpdate(), 30 * 60 * 1000);
    on('check-update', () => checkUpdate(true).catch(fail));
    appVersion().then((version) => {
        const channel = channelOfVersion(version);
        const chip = $('#channel-chip');
        chip.hidden = channel === 'stable';
        chip.textContent = channel === 'beta' ? 'Beta' : 'Alpha';
        chip.classList.toggle('alpha', channel === 'alpha');
        $('#about-version').textContent = `輝煌啟動器 v${version}（${channel === 'stable' ? '正式版' : channel === 'beta' ? 'Beta 搶先版' : 'Alpha 測試版'}）`;
    }).catch(() => {});
    let taps = 0;
    let tapTimer = 0;
    $('#about-version').addEventListener('click', () => {
        clearTimeout(tapTimer);
        tapTimer = setTimeout(() => { taps = 0; }, 1500);
        if (++taps < 7 || store.settings?.alphaUnlocked) return;
        taps = 0;
        saveSettings({ alphaUnlocked: true }).then(() => toast('已開啟 Alpha 測試版選項，可以在「設定 → 更新與安全 → 更新通道」選擇'), fail);
    });
    $('#channel-chip').addEventListener('click', async () => {
        const version = await appVersion().catch(() => '');
        const channel = channelOfVersion(version);
        const notes = {
            alpha: ['最早拿到新功能，但也最可能遇到錯誤、閃退或需要重新設定', '功能可能隨時調整、改名或移除，不保證跟之後的版本相容', '遇到問題請用「回報給管理員」，附上越詳細的資料越好', '重要的存檔請先備份，或用另一個實例測試'],
            beta: ['功能大致完成，正在做最後的測試與修正', '偶爾可能有小錯誤，通常不影響正常遊玩', '遇到問題歡迎回報，會在正式版前修好']
        }[channel] || [];
        const go = await dialog({
            title: channel === 'beta' ? 'Beta 搶先版' : 'Alpha 測試版',
            content: h('div', { class: 'channel-notes' },
                h('p', {}, `你正在使用 v${version}，屬於${channel === 'beta' ? ' Beta 搶先版' : ' Alpha 測試版'}。`),
                h('b', {}, '注意事項'),
                h('ul', {}, ...notes.map((t) => h('li', {}, t))),
                h('p', { class: 'hint' }, '正式版推出後，可以到「設定 → 更新與安全 → 更新通道」切換。更新通道只影響啟動器本身，輝煌模組不分通道。')),
            buttons: [{ label: '關閉', value: null }, { label: '前往更新設定', value: 'go' }]
        });
        if (go === 'go') $('#about-update-link').click();
    });
    on('network', () => { if (store.online !== false) checkUpdate(); });
    $('#about-update-link').addEventListener('click', (e) => {
        e.preventDefault();
        location.hash = '#settings';
        setTimeout(() => document.querySelector('#settings-nav a[data-jump="set-update"]')?.click(), 120);
    });
    startServices();

    if (!store.accounts.length) {
        toast('第一次使用：請先到「帳號」頁登入，再到「實例管理」建立實例。');
    }
}

boot();
