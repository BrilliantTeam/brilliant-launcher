import { api, openUrl } from './api.js';
import { icon } from './icons.js';
import { $, $$, clear, fail, h, toast } from './ui.js';
import { emit, on, saveSettings, selectedInstance, store } from './store.js';

const mac = navigator.userAgent.includes('Mac');

const VIEW_LABELS = { home: '首頁', profile: '我的輝煌', album: '相簿與回顧', instances: '實例管理', explore: '探索', accounts: '帳號', settings: '設定', log: '遊戲日誌', about: '關於' };

const hooks = {};

export function provide(name, fn) {
    hooks[name] = fn;
}

async function latestScreenshot() {
    const list = store.instances.length > 12 ? [selectedInstance()].filter(Boolean) : store.instances;
    const shots = (await Promise.all(list.map((i) => api.list_screenshots({ id: i.id }).catch(() => [])))).flat();
    const newest = shots.sort((a, b) => b.time - a.time)[0];
    if (!newest) return toast('還沒有任何截圖', 'warn');
    await api.open_path({ path: newest.path }).catch(fail);
}

function playLast() {
    const last = [...store.instances].sort((a, b) => (b.lastPlayed || 0) - (a.lastPlayed || 0))[0];
    if (!last) return toast('還沒有任何實例', 'warn');
    hooks.playInstance?.(last);
}

export const ACTIONS = [
    ['palette', '開啟指令面板', 'Ctrl+K', () => openPalette()],
    ['play', '開始遊戲（目前選擇的實例）', 'Ctrl+Enter', () => { location.hash = '#home'; emit('launch'); }],
    ['play-last', '啟動上次遊玩的實例', '', playLast],
    ['settings', '開啟設定', 'Ctrl+,', () => { location.hash = '#settings'; }],
    ['search', '搜尋（設定頁搜尋設定，其他頁搜尋模組）', 'Ctrl+F', () => hooks.search?.()],
    ...Object.entries(VIEW_LABELS).map(([view, label], i) => [`view-${view}`, `切換到「${label}」`, i < 9 ? `Ctrl+${i + 1}` : '', () => { location.hash = `#${view}`; }]),
    ['open-screenshot', '開啟最新截圖', '', latestScreenshot],
    ['open-folder', '開啟目前實例的資料夾', '', () => { const i = selectedInstance(); if (i) api.open_instance_folder({ id: i.id, sub: '' }).then((path) => api.open_path({ path })).catch(fail); }],
    ['next-track', '背景音樂下一首', '', () => hooks.nextTrack?.()],
    ['toggle-theme', '切換亮色／深色', '', () => $('#theme-toggle').click()],
    ['toggle-font', '切換字型', '', () => $('#font-toggle').click()],
    ['streamer', '切換直播模式', '', () => saveSettings({ streamerMode: !store.settings?.streamerMode }).then(() => toast(store.settings.streamerMode ? '已開啟直播模式：帳號、信箱、IP 與路徑會被隱藏' : '已關閉直播模式'), fail)],
    ['home-edit', '編輯主頁版面', '', () => { location.hash = '#home'; hooks.editHome?.(); }]
].map(([id, label, combo, run]) => ({ id, label, combo, run }));

export const comboOf = (id) => {
    const custom = store.settings?.shortcuts?.[id];
    return custom !== undefined ? custom : ACTIONS.find((a) => a.id === id)?.combo ?? '';
};

const KEY_NAMES = { Comma: ',', Period: '.', Slash: '/', Semicolon: ';', Quote: "'", BracketLeft: '[', BracketRight: ']', Backslash: '\\', Minus: '-', Equal: '=', Backquote: '`', ArrowUp: 'Up', ArrowDown: 'Down', ArrowLeft: 'Left', ArrowRight: 'Right', NumpadEnter: 'Enter' };

export function comboFromEvent(e) {
    const code = e.code;
    let key = KEY_NAMES[code] ?? (/^Key([A-Z])$/.exec(code)?.[1]) ?? (/^(?:Digit|Numpad)(\d)$/.exec(code)?.[1]) ?? (/^F\d{1,2}$/.test(code) ? code : null);
    if (!key && ['Enter', 'Space', 'Tab', 'Delete', 'Home', 'End', 'PageUp', 'PageDown', 'Insert'].includes(code)) key = code;
    if (!key) return null;
    const ctrl = mac ? e.metaKey : e.ctrlKey;
    return [ctrl && 'Ctrl', e.altKey && 'Alt', e.shiftKey && 'Shift', key].filter(Boolean).join('+');
}

export const comboLabel = (combo) => (combo ? combo.split('+').map((part) => (part === 'Ctrl' && mac ? '⌘' : part)).join(' + ') : '未設定');

const typing = (el) => el?.matches?.('input:not([type="checkbox"]):not([type="range"]), textarea, select, [contenteditable]');

let capturing = false;

function dispatch(e) {
    if (capturing || e.isComposing || e.keyCode === 229 || e.defaultPrevented) return;
    const combo = comboFromEvent(e);
    if (!combo) return;
    if (typing(e.target) && !/^(Ctrl|Alt)\+/.test(combo) && !/^F\d/.test(combo)) return;
    if (document.querySelector('dialog[open]') && combo !== comboOf('palette')) return;
    const action = ACTIONS.find((a) => comboOf(a.id) === combo);
    if (!action) return;
    e.preventDefault();
    action.run();
}

export function captureCombo(button) {
    return new Promise((resolve) => {
        capturing = true;
        const before = button.textContent;
        button.textContent = '請按下組合鍵⋯（Esc 取消、Backspace 清除）';
        button.classList.add('capturing');
        const finish = (value) => {
            capturing = false;
            removeEventListener('keydown', onKey, true);
            button.classList.remove('capturing');
            button.textContent = before;
            resolve(value);
        };
        const onKey = (e) => {
            e.preventDefault();
            e.stopPropagation();
            if (['Control', 'Shift', 'Alt', 'Meta'].includes(e.key)) return;
            if (e.key === 'Escape') return finish(null);
            if (e.key === 'Backspace' || e.key === 'Delete') return finish('');
            const combo = comboFromEvent(e);
            if (combo) finish(combo);
        };
        addEventListener('keydown', onKey, true);
    });
}

let registered = '';

async function applyGlobalShortcut() {
    const gs = window.__TAURI__?.globalShortcut;
    const want = store.settings?.globalShortcut || '';
    if (!gs || want === registered) return;
    if (registered) await gs.unregister(accelerator(registered)).catch(() => {});
    registered = '';
    if (!want) return;
    try {
        await gs.register(accelerator(want), (event) => {
            if (event?.state && event.state !== 'Pressed') return;
            toast('已透過系統快捷鍵開始遊戲');
            emit('launch');
        });
        registered = want;
    } catch {
        toast(`系統快捷鍵「${comboLabel(want)}」無法註冊，可能已被其他程式占用，請換一組`, 'warn');
    }
}

const ACCEL_KEYS = { ',': 'Comma', '.': 'Period', '/': 'Slash', ';': 'Semicolon', "'": 'Quote', '[': 'BracketLeft', ']': 'BracketRight', '\\': 'Backslash', '-': 'Minus', '=': 'Equal', '`': 'Backquote' };

function accelerator(combo) {
    return combo.split('+').map((part) => (part === 'Ctrl' ? 'CommandOrControl' : ACCEL_KEYS[part] ?? part)).join('+');
}

export function railLinkOpen(link) {
    if (/^https?:\/\//i.test(link.target)) openUrl(link.target);
    else api.open_path({ path: link.target }).catch(fail);
}

function renderRailLinks() {
    $$('.rail .rail-custom').forEach((el) => el.remove());
    const spacer = $('.rail .rail-spacer');
    for (const link of store.settings?.railLinks ?? []) {
        const web = /^https?:\/\//i.test(link.target);
        spacer.before(h('a', {
            href: '#',
            class: 'rail-custom',
            title: `${link.name}（${web ? '開啟網頁' : '開啟資料夾'}）`,
            style: 'order:97',
            onclick: (e) => { e.preventDefault(); railLinkOpen(link); }
        }, icon(web ? 'external-link' : 'folder'), h('span', {}, [...link.name].slice(0, 4).join(''))));
    }
}

let palette = null;

function paletteItems() {
    const items = ACTIONS.filter((a) => a.id !== 'palette').map((a) => ({ label: a.label, tag: '動作', hint: comboOf(a.id) ? comboLabel(comboOf(a.id)) : '', run: a.run }));
    for (const instance of store.instances) {
        items.push({ label: `開始遊戲：${instance.name}`, tag: '實例', run: () => hooks.playInstance?.(instance) });
        items.push({ label: `開啟實例頁：${instance.name}`, tag: '實例', run: () => hooks.manageMods?.(instance) });
        items.push({ label: `開啟資料夾：${instance.name}`, tag: '實例', run: () => api.open_instance_folder({ id: instance.id, sub: '' }).then((path) => api.open_path({ path })).catch(fail) });
    }
    for (const account of store.accounts ?? []) {
        items.push({ label: `切換帳號：${account.name}`, tag: '帳號', run: () => hooks.selectAccount?.(account) });
    }
    for (const profile of store.settings?.profiles ?? []) {
        items.push({ label: `套用設定方案：${profile.name}`, tag: '方案', run: () => hooks.applyProfile?.(profile) });
    }
    for (const link of store.settings?.railLinks ?? []) {
        items.push({ label: `開啟捷徑：${link.name}`, tag: '捷徑', run: () => railLinkOpen(link) });
    }
    const seen = new Set();
    for (const label of $$('#view-settings .settings-page label, #view-settings h3.look-group')) {
        if (label.closest('.rail-editor, .media-list, #shortcut-list, #profile-list, #rail-links')) continue;
        const text = label.textContent.replace(/[：:]\s*$/, '').replace(/（[^）]*）/g, '').trim().slice(0, 40);
        if (text.length < 2 || seen.has(text)) continue;
        seen.add(text);
        items.push({ label: text, tag: '設定', run: () => hooks.revealSetting?.(label) });
    }
    return items;
}

function openPalette() {
    if (palette?.open) return palette.querySelector('input').focus();
    const input = h('input', { type: 'text', class: 'palette-input', placeholder: '搜尋動作、實例、帳號或設定⋯', 'aria-label': '搜尋指令' });
    const list = h('div', { class: 'palette-list', role: 'listbox' });
    palette = h('dialog', { class: 'palette', 'aria-label': '指令面板' }, input, list, h('div', { class: 'palette-foot' }, '↑↓ 選擇　Enter 執行　Esc 關閉'));
    document.body.append(palette);
    const items = paletteItems();
    let shown = [];
    let active = 0;
    const paint = () => {
        const words = input.value.trim().toLowerCase().split(/\s+/).filter(Boolean);
        shown = items.filter((item) => words.every((w) => `${item.label} ${item.tag}`.toLowerCase().includes(w)))
            .sort((a, b) => (words.length ? Number(!a.label.toLowerCase().startsWith(words[0])) - Number(!b.label.toLowerCase().startsWith(words[0])) : 0))
            .slice(0, 50);
        active = Math.min(active, Math.max(0, shown.length - 1));
        clear(list).append(...(shown.length ? shown.map((item, i) => h('div', {
            class: 'palette-item',
            role: 'option',
            'aria-selected': String(i === active),
            onmousemove: () => { if (active !== i) { active = i; paint(); } },
            onmousedown: (e) => { e.preventDefault(); run(item); }
        }, h('span', { class: 'palette-tag' }, item.tag), h('span', { class: 'palette-label' }, item.label), item.hint ? h('kbd', {}, item.hint) : null)) : [h('div', { class: 'palette-empty' }, '找不到符合的項目')]));
        list.children[active]?.scrollIntoView({ block: 'nearest' });
    };
    const close = () => {
        palette.close();
        palette.remove();
        palette = null;
    };
    const run = (item) => {
        close();
        Promise.resolve(item.run()).catch(fail);
    };
    input.addEventListener('input', () => { active = 0; paint(); });
    input.addEventListener('keydown', (e) => {
        if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
            e.preventDefault();
            active = (active + (e.key === 'ArrowDown' ? 1 : -1) + shown.length) % Math.max(1, shown.length);
            paint();
        } else if (e.key === 'Enter' && !e.isComposing && shown[active]) {
            e.preventDefault();
            e.stopPropagation();
            run(shown[active]);
        }
    });
    palette.addEventListener('close', () => palette?.remove());
    palette.addEventListener('click', (e) => { if (e.target === palette) close(); });
    paint();
    palette.showModal();
    input.focus();
}

export function initCommands() {
    addEventListener('keydown', dispatch);
    on('settings', () => {
        renderRailLinks();
        applyGlobalShortcut();
    });
    renderRailLinks();
    applyGlobalShortcut();
}
