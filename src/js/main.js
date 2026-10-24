import { api, appVersion, appWindow, fileSrc, listen, openUrl, setZoom } from './api.js';
import { icon } from './icons.js';
import { $, $$, confirm, dialog, fail, h, modalOpen, rangeFill, toast, guard, workPending } from './ui.js';
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
import { initTours } from './tour.js';
import { playClick } from './sound.js';
import { setWeather } from './weather.js';
import { initCommands, provide } from './commands.js';
import { initWidgets } from './widgets.js';
import { startI18n } from './i18n.js';
import { initStreamer } from './streamer.js';
import { nextTrack } from './services.js';

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

const scrollMemory = {};

function show(view) {
    const name = TITLES[view] ? view : 'home';
    const wrap = $('.page-wrap');
    if (document.body.dataset.view) scrollMemory[document.body.dataset.view] = wrap.scrollTop;
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
    wrap.scrollTop = scrollMemory[name] ?? 0;
}

const THEMES = {
    auto: { icon: 'invert', label: '主題：跟隨系統（點一下切換為亮色）' },
    light: { icon: 'sun', label: '主題：亮色（點一下切換為深色）' },
    dark: { icon: 'moon', label: '主題：深色（點一下改為跟隨系統）' },
    schedule: { icon: 'clock', label: '主題：依時段自動切換（點一下改為跟隨系統）' }
};

const HEX = /^#[0-9a-f]{6}$/i;

export const COLOR_TOKENS = ['content-bg', 'content-bg-alt', 'content-bg-sunken', 'text', 'text-subtle', 'border', 'border-soft', 'panel-face', 'chrome-bg', 'chrome-ink', 'glass-bg', 'glass-ink'];

function scheduledTheme(look) {
    const hour = new Date().getHours();
    const from = Number(look.darkFrom ?? 19);
    const to = Number(look.darkTo ?? 7);
    const dark = from === to ? false : from < to ? hour >= from && hour < to : hour >= from || hour < to;
    return dark ? 'dark' : 'light';
}

const pick = (value, allowed, fallback) => (allowed.includes(value) ? value : fallback);

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
    const effective = theme === 'schedule' ? scheduledTheme(store.settings) : theme;
    if (effective === 'auto') delete root.dataset.theme;
    else root.dataset.theme = effective;
    root.dataset.font = pick(store.settings?.font, ['pixel', 'system', 'naikai', 'serif', ...(store.settings?.customFont ? ['custom'] : [])], 'pixel');
    loadCustomFont(store.settings?.customFont);
    const toggle = $('#theme-toggle');
    toggle.replaceChildren(icon(THEMES[theme].icon));
    toggle.title = THEMES[theme].label;
    toggle.setAttribute('aria-label', THEMES[theme].label);
    const fromBg = store.settings?.accentFromBg && HEX.test(store.bgAccent ?? '') ? store.bgAccent : '';
    const accent = fromBg || (HEX.test(store.settings?.accent ?? '') ? store.settings.accent : '');
    if (accent) {
        const [r, g, b] = [1, 3, 5].map((i) => parseInt(accent.slice(i, i + 2), 16));
        root.style.setProperty('--accent', accent);
        root.style.setProperty('--accent-hover', `color-mix(in srgb, ${accent} 85%, #000)`);
        root.style.setProperty('--accent-ink', 0.299 * r + 0.587 * g + 0.114 * b > 140 ? '#111111' : '#ffffff');
    } else {
        for (const key of ['--accent', '--accent-hover', '--accent-ink']) root.style.removeProperty(key);
    }
    root.dataset.motion = store.settings?.reduceMotion ? 'off' : 'on';
    const look = store.settings ?? {};
    root.dataset.buttons = look.buttonStyle === 'flat' ? 'flat' : 'pixel';
    root.dataset.corners = look.corners === 'rounded' ? 'rounded' : 'square';
    root.dataset.cards = look.cardSize === 'compact' ? 'compact' : 'comfy';
    root.dataset.rail = look.railLabels === false ? 'icons' : 'labels';
    root.dataset.news = look.showNews === false ? 'off' : 'on';
    const alpha = Math.min(100, Math.max(60, Number(look.panelOpacity) || 100));
    root.style.setProperty('--panel-alpha', `${alpha}%`);
    root.dataset.glassy = alpha < 100 ? 'on' : 'off';
    root.style.setProperty('--bg-dim', String(Math.min(80, Math.max(0, Number(look.bgDim) || 0)) / 100));
    root.dataset.width = ['wide', 'full'].includes(look.contentWidth) ? look.contentWidth : 'standard';
    root.dataset.density = look.density === 'compact' ? 'compact' : 'comfy';
    root.dataset.bar = ['thin', 'none'].includes(look.accentBar) ? look.accentBar : 'thick';
    const blur = Math.min(20, Math.max(0, Number(look.bgBlur) || 0));
    root.style.setProperty('--bg-blur', `${blur}px`);
    root.dataset.blurred = blur > 0 ? 'on' : 'off';
    root.dataset.bgmotion = look.bgMotion ? 'on' : 'off';
    root.dataset.scroll = look.thinScrollbars ? 'thin' : 'normal';
    root.dataset.carddetails = look.cardDetails === false ? 'off' : 'on';
    root.dataset.brand = look.showBrand === false ? 'off' : 'on';
    root.dataset.railstyle = ['solid', 'accent'].includes(look.railStyle) ? look.railStyle : 'glass';
    root.dataset.cover = ['short', 'tall'].includes(look.coverHeight) ? look.coverHeight : 'normal';
    root.dataset.cursor = look.cursorStyle === 'pixel' ? 'pixel' : 'system';
    root.dataset.layout = look.homeLayout === 'mirrored' ? 'mirrored' : 'standard';
    const tones = { mono: 'grayscale(1)', sepia: 'sepia(0.75) saturate(1.1)', cool: 'saturate(0.9) hue-rotate(12deg) brightness(0.97)', warm: 'sepia(0.25) saturate(1.3)', vivid: 'saturate(1.5) contrast(1.08)' };
    root.style.setProperty('--bg-tone', tones[look.bgFilter] ?? 'saturate(1)');
    for (const [key, cssVar] of [['colorSuccess', '--success'], ['colorWarning', '--warning'], ['colorLink', '--link']]) {
        if (HEX.test(look[key] ?? '')) root.style.setProperty(cssVar, look[key]);
        else root.style.removeProperty(cssVar);
    }
    root.dataset.contrast = look.highContrast ? 'high' : 'normal';
    root.dataset.streamer = look.streamerMode ? 'on' : 'off';
    const mode = root.dataset.theme || (matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light');
    root.dataset.mode = mode;
    for (const token of COLOR_TOKENS) {
        const value = look.colorOverrides?.[`${mode}.${token}`];
        if (HEX.test(value ?? '')) root.style.setProperty(`--${token}`, value);
        else root.style.removeProperty(`--${token}`);
    }
    root.dataset.heading = look.headingStyle === 'modern' ? 'modern' : 'game';
    root.style.setProperty('--fs', String(Math.min(130, Math.max(90, Number(look.fontScale) || 100)) / 100));
    const solid = look.bgSource === 'color';
    root.dataset.bgsource = solid ? 'color' : 'auto';
    const c1 = HEX.test(look.bgColor ?? '') ? look.bgColor : '#1b2027';
    root.style.setProperty('--bg-solid', HEX.test(look.bgColor2 ?? '') ? `linear-gradient(135deg, ${c1}, ${look.bgColor2})` : c1);
    root.dataset.bgtrans = pick(look.bgTransition, ['fade', 'slide', 'pixel', 'none'], 'fade');
    root.dataset.bgpan = look.bgPan ? 'on' : 'off';
    setWeather(look.reduceMotion ? 'none' : look.weather);
    root.dataset.clockstyle = pick(look.clockStyle, ['bold', 'thin', 'pixel'], 'bold');
    root.dataset.clockpos = look.clockPosition === 'center' ? 'center' : 'right';
    root.dataset.playsize = pick(look.playSize, ['normal', 'large', 'xl'], 'normal');
    root.dataset.shards = look.shardLayout === 'row' ? 'row' : 'grid';
    root.dataset.credit = look.showCredit === false ? 'off' : 'on';
    root.dataset.social = look.showSocial === false ? 'off' : 'on';
    root.dataset.shardstats = look.showShardStats === false ? 'off' : 'on';
    root.dataset.lastplayed = look.showLastPlayed === false ? 'off' : 'on';
    root.dataset.cardview = pick(look.cardView, ['grid', 'list', 'poster'], 'grid');
    root.dataset.cardhover = pick(look.cardHover, ['lift', 'zoom', 'none'], 'lift');
    root.style.setProperty('--cover-shade', String(Math.min(80, Math.max(0, Number(look.coverShade ?? 35))) / 100));
    root.style.setProperty('--cover-pos', pick(look.coverPosition, ['top', 'center', 'bottom'], 'center'));
    root.dataset.pagestats = look.pageStats === false ? 'off' : 'on';
    root.dataset.railside = look.railSide === 'right' ? 'right' : 'left';
    const order = Array.isArray(look.railOrder) ? look.railOrder : [];
    const hiddenViews = Array.isArray(look.railHidden) ? look.railHidden : [];
    $$('.rail a[data-view]').forEach((link, i) => {
        const view = link.dataset.view;
        link.style.order = view === 'about' ? '99' : String(order.includes(view) ? order.indexOf(view) : order.length + i);
        link.hidden = hiddenViews.includes(view) && !['home', 'instances', 'settings', 'about'].includes(view);
    });
    root.dataset.accountname = look.hideAccountName ? 'off' : 'on';
    for (const id of ['walker-toggle', 'font-toggle', 'theme-toggle']) $(`#${id}`).hidden = (look.hiddenTools ?? []).includes(id);
    root.dataset.pagetrans = pick(look.pageTransition, ['rise', 'fade', 'slide', 'none'], 'rise');
    animRate = { fast: 1.6, slow: 0.6 }[look.animSpeed] ?? 1;
    root.dataset.toastpos = pick(look.toastPosition, ['bottom-right', 'top-right', 'top-center'], 'bottom-right');
    root.dataset.toastSeconds = String(Math.min(15, Math.max(2, Number(look.toastSeconds) || 6)));
    $('#home-clock').hidden = !look.showClock;
    tickClock();
    const tagline = $('.home-brand .tagline');
    tagline.dataset.default ??= tagline.textContent;
    tagline.textContent = (look.homeTagline || '').trim() || tagline.dataset.default;
    const scale = (store.settings?.uiScale || 100) / 100;
    if (scale !== appliedScale) {
        appliedScale = scale;
        setZoom(scale).catch(() => {});
    }
    const font = $('#font-toggle');
    const fonts = fontOrder();
    const nextFont = fonts[(fonts.indexOf(currentFont()) + 1) % fonts.length];
    font.title = `字型：${fontName(currentFont())}（點一下改為${fontName(nextFont)}）`;
    font.setAttribute('aria-label', font.title);
}

const FONT_NAMES = { pixel: '像素字型', system: '黑體', naikai: '內海字體', serif: '明體' };

const fontOrder = () => ['pixel', 'system', 'naikai', 'serif', ...(store.settings?.fonts ?? []).map((f) => `custom:${f.path}`)];

const currentFont = () => (document.documentElement.dataset.font === 'custom' ? `custom:${store.settings?.customFont}` : document.documentElement.dataset.font);

const fontName = (id) => FONT_NAMES[id] ?? `「${(store.settings?.fonts ?? []).find((f) => `custom:${f.path}` === id)?.name ?? '自訂字型'}」`;

let customFace = null;

function loadCustomFont(path) {
    if ((customFace?.path ?? '') === (path ?? '')) return;
    if (customFace) document.fonts.delete(customFace.face);
    customFace = null;
    if (!path) return;
    const face = new FontFace('BrilliantCustom', `url("${fileSrc(path)}")`);
    customFace = { path, face };
    document.fonts.add(face);
    face.load().catch(() => toast('自訂字型無法載入，可能檔案已損壞或格式不支援，請重新上傳', 'error'));
}

let animRate = 1;

function speedAnimations(e) {
    if (animRate === 1 || !e.target.getAnimations) return;
    for (const animation of e.target.getAnimations()) animation.playbackRate = animRate;
}

let lastScheduled = '';

function tickClock() {
    const look = store.settings ?? {};
    if (look.theme === 'schedule' && scheduledTheme(look) !== lastScheduled) {
        lastScheduled = scheduledTheme(look);
        applyTheme();
    }
    if ($('#home-clock').hidden || document.hidden) return;
    const now = new Date();
    $('#clock-time').textContent = now.toLocaleTimeString('zh-TW', { hour: '2-digit', minute: '2-digit', second: look.clockSeconds ? '2-digit' : undefined, hour12: look.clockFormat === '12' });
    $('#clock-date').textContent = now.toLocaleDateString('zh-TW', { month: 'long', day: 'numeric', weekday: 'long' });
}

function clickSound(e) {
    if (!store.settings?.clickSound || e.button !== 0 || !e.target.closest?.('button:not(:disabled), .rail a, [role="menuitem"], .look-preset')) return;
    playClick(store.settings.clickSoundType, store.settings.clickVolume ?? 50);
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
    let closing = false;
    const closeWindow = async () => {
        if (store.settings?.closeToTray) return win.hide();
        if (closing) return;
        closing = true;
        const busy = (workPending() && !modalOpen()) || !$('#progress').hidden;
        const go = !busy || await confirm('還有工作正在進行（例如下載、複製或匯入），現在關閉會讓它中斷，可能留下不完整的檔案。\n仍要關閉輝煌啟動器嗎？', '強制關閉');
        closing = false;
        if (go) await win.destroy();
    };
    $('#win-close').addEventListener('click', closeWindow);
    win.onCloseRequested?.((event) => {
        event.preventDefault();
        closeWindow();
    });
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

    const mac = navigator.userAgent.includes('Mac');
    if (mac) $$('kbd.mod-key').forEach((k) => { k.textContent = '⌘'; });
    document.addEventListener('keydown', (e) => {
        if (e.key === 'F5' || e.key === 'BrowserRefresh') e.preventDefault();
    });
    document.addEventListener('keydown', (e) => {
        if ((mac ? e.metaKey : e.ctrlKey) && (e.code === 'KeyR' || e.code === 'KeyP')) e.preventDefault();
    });
    provide('search', () => {
        if (location.hash === '#settings') {
            $('#settings-search').focus();
            $('#settings-search').select();
            return;
        }
        location.hash = '#mods';
        setTimeout(() => $('#mod-search').focus(), 50);
    });
    provide('nextTrack', nextTrack);

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

    $('#font-toggle').addEventListener('click', () => {
        const order = fontOrder();
        const next = order[(order.indexOf(currentFont()) + 1) % order.length];
        const patch = next.startsWith('custom:') ? { font: 'custom', customFont: next.slice(7) } : { font: next };
        saveSettings(patch).then(() => toast(`已切換為${fontName(next)}`), fail);
    });

    document.addEventListener('click', (e) => {
        const link = e.target.closest('a[data-url]');
        if (!link) return;
        e.preventDefault();
        openUrl(link.dataset.url);
    });

    addEventListener('hashchange', () => {
        show(location.hash.slice(1));
        try {
            localStorage.setItem(LAST_VIEW_KEY, document.body.dataset.view);
        } catch {}
    });
}

const LAST_VIEW_KEY = 'brilliant.lastView';

function startView() {
    const start = store.settings?.startPage;
    let view = start === 'instances' ? 'instances' : 'home';
    if (start === 'last') {
        try {
            view = localStorage.getItem(LAST_VIEW_KEY) || 'home';
        } catch {}
    }
    if (!TITLES[view] || view === 'mods') view = 'home';
    if (view !== 'home') history.replaceState(null, '', `#${view}`);
    return view;
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

async function downloadUpdate(manual = false) {
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
        if (manual || /簽章/.test(String(error?.message ?? error))) fail(error);
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
        else if (go === 'retry') downloadUpdate(true);
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
    await startI18n(store.settings?.language).catch(() => {});
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
    initTours();
    initCommands();
    initWidgets();
    initStreamer();
    setInterval(tickClock, 1000);
    matchMedia('(prefers-color-scheme: dark)').addEventListener('change', applyTheme);
    addEventListener('pointerdown', clickSound, true);
    document.addEventListener('animationstart', speedAnimations, true);
    document.addEventListener('transitionrun', speedAnimations, true);
    on('bg-color', () => {
        if (store.settings?.accentFromBg) applyTheme();
    });

    emit('settings');
    show(location.hash.slice(1) || startView());
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
