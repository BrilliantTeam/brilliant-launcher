import { api, appVersion, appWindow, openUrl, setZoom } from './api.js';
import { icon } from './icons.js';
import { $, $$, dialog, fail, h, rangeFill, toast } from './ui.js';
import { emit, loadAccounts, loadInstances, loadSettings, on, store } from './store.js';
import { initHome } from './views/home.js';
import { initInstances } from './views/instances.js';
import { initMods } from './views/mods.js';
import { initAccounts } from './views/accounts.js';
import { initSettings } from './views/settings.js';
import { initProfile } from './views/profile.js';
import { notifyOnce, startServices } from './services.js';
import { initLog } from './views/logview.js';

const TITLES = {
    profile: '我的輝煌',
    home: '首頁',
    instances: '實例管理',
    mods: '模組與資源',
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
    $$('.rail a').forEach((el) => el.classList.toggle('active', el.dataset.view === name));
    $('.page-wrap').scrollTop = 0;
}

const THEMES = {
    auto: { icon: 'invert', label: '主題：跟隨系統（點一下切換為亮色）' },
    light: { icon: 'sun', label: '主題：亮色（點一下切換為深色）' },
    dark: { icon: 'moon', label: '主題：深色（點一下改為跟隨系統）' }
};

let appliedScale = 1;

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

    const views = $$('.rail a[data-view]').map((a) => a.dataset.view);
    document.addEventListener('keydown', (e) => {
        if (!e.ctrlKey || e.altKey || e.metaKey) return;
        if (e.key === 'Enter') {
            e.preventDefault();
            location.hash = '#home';
            emit('launch');
        } else if (/^[1-9]$/.test(e.key) && views[e.key - 1]) {
            e.preventDefault();
            location.hash = `#${views[e.key - 1]}`;
        } else if (e.key === ',') {
            e.preventDefault();
            location.hash = '#settings';
        } else if (e.key.toLowerCase() === 'f') {
            e.preventDefault();
            location.hash = '#mods';
            setTimeout(() => $('#mod-search').focus(), 50);
        } else if (e.key.toLowerCase() === 'r' || e.key === 'p') {
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

export function newerVersion(latest, current) {
    const a = String(latest).split('.').map(Number);
    const b = String(current).split('.').map(Number);
    for (let i = 0; i < 3; i++) {
        if ((a[i] || 0) !== (b[i] || 0)) return (a[i] || 0) > (b[i] || 0);
    }
    return false;
}

async function checkUpdate() {
    const [info, current] = await Promise.all([api.launcher_info().catch(() => null), appVersion().catch(() => '')]);
    const latest = info?.launcher;
    if (!latest || !current || !newerVersion(latest.version, current)) return;
    notifyOnce(`launcher:${latest.version}`, `輝煌啟動器 v${latest.version} 已推出`, '打開啟動器，點右上角的更新按鈕即可升級。');
    const chip = $('#update-chip');
    $('#update-label').textContent = `更新 v${latest.version}`;
    chip.hidden = false;
    chip.addEventListener('click', async () => {
        const go = await dialog({
            title: `輝煌啟動器 v${latest.version}`,
            content: h('div', {},
                h('p', { class: 'hint' }, `目前版本 v${current}`),
                h('p', { style: 'white-space:pre-wrap' }, latest.notes || '有新版本可以更新。')),
            buttons: [{ label: '稍後', value: false }, { label: '立即更新', value: true }]
        });
        if (go !== true) return;
        if (!latest.sha256) return openUrl(latest.page);
        toast('正在下載新版本，完成後會自動開啟安裝程式⋯');
        await api.install_update({ url: latest.url, file: latest.file, sha256: latest.sha256 }).catch((error) => {
            fail(error);
            openUrl(latest.page);
        });
    });
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

    await Promise.all([loadInstances().catch(fail), loadAccounts().catch(fail)]);

    initHome();
    initInstances();
    initMods();
    initAccounts();
    initSettings();
    initProfile();
    initLog();

    emit('settings');
    show(location.hash.slice(1) || 'home');
    checkUpdate();
    startServices();

    if (!store.accounts.length) {
        toast('第一次使用：請先到「帳號」頁登入，再到「實例管理」建立實例。');
    }
}

boot();
