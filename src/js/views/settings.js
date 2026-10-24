import { api, fileSrc, openUrl, pickFile } from '../api.js';
import { $, $$, acceptFiles, clampNumber, clear, confirm, dialog, fail, fmtBytes, h, pickColor, rangeFill, toast, guard } from '../ui.js';
import { emit, loadInstances, loadSettings, on, saveSettings, store } from '../store.js';
import { icon } from '../icons.js';
import { gpuName, isIntegratedGpu, nextTrack, nowPlaying } from '../services.js';
import { SHARDS } from './home.js';
import { ACTIONS, captureCombo, comboLabel, comboOf, provide, railLinkOpen } from '../commands.js';
import { playClick } from '../sound.js';

const FIELDS = {
    memoryMb: '#set-memory',
    width: '#set-width',
    height: '#set-height',
    fullscreen: '#set-fullscreen',
    jvmArgs: '#set-jvm',
    quickJoin: '#set-quick-join',
    javaPath: '#set-java',
    javaAutoUpdate: '#set-java-auto',
    useMirror: '#set-mirror',
    showSnapshots: '#set-snapshots',
    showOld: '#set-old',
    injectRichPresence: '#set-rpc',
    afterLaunch: '#set-after',
    backupOnLaunch: '#set-backup',
    backupKeep: '#set-backup-keep',
    preLaunchCmd: '#set-pre',
    postExitCmd: '#set-post',
    slideSeconds: '#set-slide',
    uiScale: '#set-scale',
    reduceMotion: '#set-motion',
    walkers: '#set-walkers',
    closeToTray: '#set-tray',
    autostart: '#set-autostart',
    downloadThreads: '#set-threads',
    notifications: '#set-notify',
    usageStats: '#set-usage',
    inheritOptions: '#set-inherit',
    musicVolume: '#set-volume',
    musicMode: '#set-music-mode',
    memoryMinMb: '#set-memory-min',
    wrapper: '#set-wrapper',
    envVars: '#set-env',
    preferDgpu: '#set-dgpu',
    modChannel: '#set-mod-channel',
    autoUpdateMods: '#set-mod-auto',
    proxy: '#set-proxy',
    debugMode: '#set-debug',
    openLogOnLaunch: '#set-open-log',
    launcherUpdate: '#set-launcher-update',
    updateChannel: '#set-update-channel',
    modUpdate: '#set-mod-update',
    buttonStyle: '#set-buttons',
    corners: '#set-corners',
    cardSize: '#set-cards',
    panelOpacity: '#set-panel',
    bgDim: '#set-dim',
    railLabels: '#set-rail-labels',
    showNews: '#set-news',
    contentWidth: '#set-content-width',
    density: '#set-density',
    accentBar: '#set-accent-bar',
    bgBlur: '#set-blur',
    bgMotion: '#set-bg-motion',
    thinScrollbars: '#set-thin-scroll',
    cardDetails: '#set-card-details',
    showBrand: '#set-brand',
    homeTagline: '#set-tagline',
    railStyle: '#set-rail-style',
    coverHeight: '#set-cover-height',
    cursorStyle: '#set-cursor',
    bgFilter: '#set-bg-filter',
    homeLayout: '#set-home-layout',
    showClock: '#set-clock',
    clickSound: '#set-click-sound',
    theme: '#set-theme',
    darkFrom: '#set-dark-from',
    darkTo: '#set-dark-to',
    accentFromBg: '#set-accent-bg',
    highContrast: '#set-contrast',
    font: '#set-font',
    headingStyle: '#set-heading',
    fontScale: '#set-font-scale',
    bgSource: '#set-bg-source',
    bgTransition: '#set-bg-transition',
    bgPan: '#set-bg-pan',
    weather: '#set-weather',
    shardLayout: '#set-shard-layout',
    playSize: '#set-play-size',
    playText: '#set-play-text',
    showCredit: '#set-credit',
    showSocial: '#set-social',
    showShardStats: '#set-shard-stats',
    showLastPlayed: '#set-last-played',
    clockFormat: '#set-clock-format',
    clockStyle: '#set-clock-style',
    clockPosition: '#set-clock-position',
    clockSeconds: '#set-clock-seconds',
    cardView: '#set-card-view',
    instanceSort: '#set-inst-sort',
    cardHover: '#set-card-hover',
    coverPosition: '#set-cover-pos',
    coverShade: '#set-cover-shade',
    pageStats: '#set-page-stats',
    railSide: '#set-rail-side',
    startPage: '#set-start-page',
    animSpeed: '#set-anim-speed',
    pageTransition: '#set-page-transition',
    clickSoundType: '#set-click-type',
    clickVolume: '#set-click-volume',
    toastPosition: '#set-toast-pos',
    toastSeconds: '#set-toast-seconds',
    launchSplash: '#set-launch-splash',
    streamerMode: '#set-streamer',
    windowMode: '#set-window-mode',
    windowMonitor: '#set-window-monitor',
    rememberWindow: '#set-remember-window',
    optionsSync: '#set-options-sync',
    modSnapshots: '#set-mod-snapshots',
    countdownTitle: '#set-countdown-title',
    language: '#set-language'
};

const LOOK_PRESETS = [
    ['輝煌經典', 'Minecraft 方塊按鈕、金色邊條，原汁原味', {}],
    ['現代玻璃', '扁平按鈕、圓角、霧化面板與緩慢縮放的背景', { buttonStyle: 'flat', corners: 'rounded', panelOpacity: 75, bgDim: 20, bgBlur: 4, bgMotion: true, accentBar: 'thin' }],
    ['極簡', '收起標誌與公告，只留下必要的資訊', { buttonStyle: 'flat', corners: 'rounded', accentBar: 'none', showNews: false, showBrand: false, railLabels: false, cardDetails: false }],
    ['緊湊效率', '寬版、緊湊間距，一次看到更多實例與模組', { contentWidth: 'wide', density: 'compact', cardSize: 'compact', railLabels: false, thinScrollbars: true, coverHeight: 'short' }],
    ['像素懷舊', '方塊按鈕、懷舊色調、像素游標與點擊音效', { bgFilter: 'sepia', cursorStyle: 'pixel', clickSound: true, railStyle: 'solid', coverHeight: 'tall', bgDim: 15 }]
];

const LOOK_DEFAULTS = {
    accent: '', buttonStyle: 'pixel', corners: 'square', cardSize: 'comfy', panelOpacity: 100, bgDim: 0, railLabels: true, showNews: true, uiScale: 100, reduceMotion: false,
    contentWidth: 'standard', density: 'comfy', accentBar: 'thick', bgBlur: 0, bgMotion: false, thinScrollbars: false, cardDetails: true, showBrand: true, homeTagline: '',
    railStyle: 'glass', coverHeight: 'normal', cursorStyle: 'system', bgFilter: 'none', homeLayout: 'standard', showClock: false, clickSound: false,
    theme: 'auto', darkFrom: 19, darkTo: 7, accentFromBg: false, colorSuccess: '', colorWarning: '', colorLink: '', highContrast: false,
    font: 'pixel', headingStyle: 'game', fontScale: 100, slideSeconds: 15,
    bgSource: 'auto', bgColor: '', bgColor2: '', bgPin: '', bgTransition: 'fade', bgPan: false, weather: 'none',
    shardLayout: 'grid', hiddenShards: [], playSize: 'normal', playText: '', showCredit: true, showSocial: true, showShardStats: true, showLastPlayed: true,
    clockFormat: '24', clockStyle: 'bold', clockPosition: 'right', clockSeconds: false,
    cardView: 'grid', instanceSort: 'manual', cardHover: 'lift', coverPosition: 'center', coverShade: 35, pageStats: true,
    railSide: 'left', railOrder: [], railHidden: [], hideAccountName: false, hiddenTools: [], startPage: 'home',
    animSpeed: 'normal', pageTransition: 'rise', clickSoundType: 'block', clickVolume: 50, toastPosition: 'bottom-right', toastSeconds: 6, launchSplash: false
};

const STYLE_KEYS = ['buttonStyle', 'corners', 'cardSize', 'panelOpacity', 'bgDim', 'railLabels', 'showNews', 'contentWidth', 'density', 'accentBar', 'bgBlur', 'bgMotion',
    'thinScrollbars', 'cardDetails', 'showBrand', 'railStyle', 'coverHeight', 'cursorStyle', 'bgFilter', 'homeLayout', 'showClock', 'clickSound', 'accentFromBg', 'highContrast',
    'headingStyle', 'bgTransition', 'bgPan', 'weather', 'shardLayout', 'playSize', 'clockStyle', 'clockPosition', 'cardView', 'cardHover', 'coverPosition', 'coverShade',
    'pageTransition', 'clickSoundType', 'launchSplash'];

const LOOK_LOCAL = ['bgPin', 'railOrder', 'railHidden', 'hiddenTools', 'hiddenShards', 'startPage', 'hideAccountName', 'uiScale', 'reduceMotion'];

function cleanLook(raw) {
    const look = {};
    for (const [key, value] of Object.entries(raw && typeof raw === 'object' ? raw : {})) {
        const fallback = LOOK_DEFAULTS[key];
        if (fallback === undefined) continue;
        if (Array.isArray(fallback) ? Array.isArray(value) && value.every((v) => typeof v === 'string') : typeof value === typeof fallback) look[key] = value;
    }
    return look;
}

async function applyLook(raw, name) {
    const base = Object.fromEntries(STYLE_KEYS.map((key) => [key, LOOK_DEFAULTS[key]]));
    await persist({ ...base, ...cleanLook(raw) });
    toast(`已套用「${name}」風格`);
}

const lookCard = (name, hint, look, by = '') => h('button', { type: 'button', class: 'look-preset', onclick: () => applyLook(look, name) },
    h('b', {}, name, by ? h('span', { class: 'look-author' }, by) : null), h('small', {}, hint));

function renderLookPresets() {
    clear($('#look-presets')).append(...LOOK_PRESETS.map(([name, hint, patch]) => lookCard(name, hint, patch)));
}

async function renderCommunityLooks() {
    const box = $('#look-community');
    const data = await api.site_json({ path: '/api/launcher/looks' }).catch(() => null);
    const looks = Array.isArray(data?.looks) ? data.looks.slice(0, 30) : null;
    clear(box).append(...(looks?.length
        ? looks.map((item) => lookCard(String(item.name).slice(0, 24), String(item.description ?? '').slice(0, 80), item.look, String(item.author ?? '').slice(0, 32)))
        : [h('div', { class: 'hint' }, looks ? '目前還沒有社群風格。' : '暫時連不上官網，稍後再回來看看。')]));
}

const LOOK_CODE = 'BRLOOK1:';

function exportLook() {
    const settings = store.settings ?? {};
    const look = Object.fromEntries(Object.keys(LOOK_DEFAULTS).filter((key) => !LOOK_LOCAL.includes(key) && settings[key] !== undefined).map((key) => [key, settings[key]]));
    const bytes = new TextEncoder().encode(JSON.stringify(look));
    const code = LOOK_CODE + btoa(String.fromCharCode(...bytes));
    const text = h('textarea', { readonly: true, spellcheck: 'false', class: 'look-code' }, code);
    navigator.clipboard?.writeText(code).then(() => toast('已複製外觀代碼'), () => {});
    dialog({
        title: '分享我的外觀',
        content: h('div', { class: 'field' },
            h('label', {}, '外觀代碼（已複製）'), text,
            h('div', { class: 'hint' }, '把代碼貼給朋友，對方在「匯入外觀」貼上就能套用；也可以分享到輝煌 Discord，精選作品會上架到「社群風格」。背景圖片、音樂與側欄排序不會包含在代碼裡。')),
        buttons: [{ label: '關閉', value: null }]
    });
    text.select();
}

async function importLook() {
    const text = h('textarea', { spellcheck: 'false', class: 'look-code', placeholder: `${LOOK_CODE}⋯` });
    const ok = await dialog({
        title: '匯入外觀',
        content: h('div', { class: 'field' }, h('label', {}, '貼上外觀代碼'), text),
        buttons: [{ label: '取消', value: false }, { label: '套用', value: true }]
    });
    if (ok !== true) return;
    const code = text.value.trim();
    let look = null;
    try {
        if (!code.startsWith(LOOK_CODE)) throw new Error();
        const bytes = Uint8Array.from(atob(code.slice(LOOK_CODE.length)), (c) => c.charCodeAt(0));
        look = cleanLook(JSON.parse(new TextDecoder().decode(bytes)));
    } catch {
        return toast(`這不是有效的外觀代碼，代碼應該以「${LOOK_CODE}」開頭`, 'error');
    }
    if (!Object.keys(look).length) return toast('代碼裡沒有可以套用的外觀設定', 'warn');
    await persist(look);
    toast('已匯入外觀');
}

const previewFamilies = new Map();

function previewFamily(path) {
    if (!previewFamilies.has(path)) {
        const family = `BrilliantPreview${previewFamilies.size}`;
        const face = new FontFace(family, `url("${fileSrc(path)}")`);
        document.fonts.add(face);
        face.load().catch(() => {});
        previewFamilies.set(path, family);
    }
    return previewFamilies.get(path);
}

const fileName = (path) => path.split(/[\\/]/).pop().replace(/\.[^.]+$/, '');

async function removeMedia(key, item) {
    if (!(await confirm(`要移除「${item.name}」嗎？檔案會移到回收資料夾，30 天內可以找回。`, '移除'))) return;
    try {
        await api.remove_media({ path: item.path });
    } catch (error) {
        return fail(error);
    }
    const list = (store.settings?.[key] ?? []).filter((x) => x.path !== item.path);
    const patch = { [key]: list };
    if (key === 'fonts' && store.settings?.customFont === item.path) {
        patch.customFont = list[0]?.path ?? '';
        if (!list.length && store.settings?.font === 'custom') patch.font = 'pixel';
    }
    if (key === 'playlist' && store.settings?.music === item.path) patch.music = list[0]?.path ?? '';
    await persist(patch);
}

function renderMedia() {
    const settings = store.settings ?? {};
    const fonts = settings.fonts ?? [];
    const usingFont = settings.font === 'custom' ? settings.customFont : '';
    clear($('#font-list')).append(...(fonts.length ? fonts.map((item) => h('div', { class: `media-row${item.path === usingFont ? ' current' : ''}` },
        h('span', { class: 'media-name' }, item.name, h('small', { style: `font-family:"${previewFamily(item.path)}", sans-serif` }, '輝煌 Brilliant 123')),
        item.path === usingFont ? h('span', { class: 'badge' }, '使用中') : h('button', { type: 'button', class: 'mc', onclick: () => persist({ font: 'custom', customFont: item.path }) }, '使用'),
        h('button', { type: 'button', class: 'mc icon-btn danger', title: '移除', 'aria-label': `移除「${item.name}」`, onclick: () => removeMedia('fonts', item) }, icon('trash')))) : [h('div', { class: 'hint' }, '還沒有自訂字型。')]));
    const tracks = settings.playlist ?? [];
    const now = nowPlaying();
    const move = (i, step) => {
        const next = [...tracks];
        [next[i], next[i + step]] = [next[i + step], next[i]];
        persist({ playlist: next });
    };
    clear($('#music-list')).append(...(tracks.length ? tracks.map((item, i) => h('div', { class: `media-row${item.path === now ? ' current' : ''}` },
        h('span', { class: 'media-name' }, item.path === now ? icon('music') : null, item.name),
        item.path === now ? h('span', { class: 'badge' }, '播放中') : h('button', { type: 'button', class: 'mc', onclick: () => persist({ music: item.path }) }, '播放'),
        h('button', { type: 'button', class: 'mc icon-btn', title: '往上移', 'aria-label': `${item.name}往上移`, disabled: i === 0, onclick: () => move(i, -1) }, icon('arrow-up')),
        h('button', { type: 'button', class: 'mc icon-btn', title: '往下移', 'aria-label': `${item.name}往下移`, disabled: i === tracks.length - 1, onclick: () => move(i, 1) }, icon('arrow-down')),
        h('button', { type: 'button', class: 'mc icon-btn danger', title: '移除', 'aria-label': `移除「${item.name}」`, onclick: () => removeMedia('playlist', item) }, icon('trash')))) : [h('div', { class: 'hint' }, '還沒有背景音樂。')]));
    $('#music-state').textContent = tracks.length ? `共 ${tracks.length} 首` : '';
    $('#btn-music-next').disabled = tracks.length < 2;
}

async function addMedia(kind, paths) {
    const list = [paths].flat().filter(Boolean);
    if (!list.length) return [];
    const added = [];
    for (const path of list) {
        try {
            const saved = await api.set_media({ kind, path });
            if (kind === 'font') await new FontFace('BrilliantFontCheck', `url("${fileSrc(saved)}")`).load();
            added.push({ path: saved, name: fileName(path).slice(0, 80) });
        } catch (error) {
            fail(error instanceof DOMException ? `「${fileName(path)}」無法載入，可能已損壞或不是字型檔` : error);
        }
    }
    return added;
}

export function revealSetting(target) {
    location.hash = '#settings';
    const page = target.closest('.settings-page')?.dataset.page;
    if (page) $(`#settings-nav [data-jump="${page}"]`)?.click();
    const block = target.closest('[data-group]');
    if (block) $(`.look-tabs button[data-group="${block.dataset.group}"]`)?.click();
    setTimeout(() => {
        const box = target.closest('.field, .field-row, h3') ?? target;
        box.scrollIntoView({ block: 'center' });
        box.classList.remove('flash');
        void box.offsetWidth;
        box.classList.add('flash');
        (box.querySelector('input, select, button') ?? box).focus?.({ preventScroll: true });
    }, 80);
}

function renderShortcuts() {
    $('#btn-global-shortcut').textContent = comboLabel(store.settings?.globalShortcut || '');
    clear($('#shortcut-list')).append(...ACTIONS.map((action) => {
        const combo = comboOf(action.id);
        const custom = store.settings?.shortcuts?.[action.id] !== undefined;
        return h('div', { class: 'shortcut-row' },
            h('span', { class: 'grow' }, action.label),
            h('button', {
                type: 'button',
                class: `mc combo-btn${combo ? '' : ' empty'}`,
                onclick: async (e) => {
                    const next = await captureCombo(e.currentTarget);
                    if (next === null) return;
                    const clash = next && ACTIONS.find((other) => other.id !== action.id && comboOf(other.id) === next);
                    const shortcuts = { ...(store.settings?.shortcuts ?? {}), [action.id]: next };
                    if (clash) shortcuts[clash.id] = '';
                    await persist({ shortcuts });
                    if (clash) toast(`「${comboLabel(next)}」原本是「${clash.label}」的快捷鍵，已改給「${action.label}」`, 'warn');
                }
            }, comboLabel(combo)),
            custom ? h('button', { type: 'button', class: 'mc icon-btn reset-btn', title: '還原成預設快捷鍵', 'aria-label': `把「${action.label}」還原成預設快捷鍵`, onclick: () => {
                const shortcuts = { ...(store.settings?.shortcuts ?? {}) };
                delete shortcuts[action.id];
                persist({ shortcuts });
            } }, icon('reload')) : null);
    }));
}

function renderRailLinks() {
    const links = store.settings?.railLinks ?? [];
    clear($('#rail-links')).append(...links.map((link, i) => h('div', { class: 'media-row' },
        h('span', { class: 'media-name' }, icon(/^https?:/i.test(link.target) ? 'external-link' : 'folder'), link.name, h('small', { class: 'sensitive' }, link.target)),
        h('button', { type: 'button', class: 'mc', onclick: () => railLinkOpen(link) }, '開啟'),
        h('button', { type: 'button', class: 'mc icon-btn danger', title: '移除', 'aria-label': `移除「${link.name}」`, onclick: () => persist({ railLinks: links.filter((_, j) => j !== i) }) }, icon('trash')))));
    $('#btn-rail-link-add').disabled = links.length >= 8;
}

const PROFILE_SKIP = ['selectedAccount', 'selectedInstance', 'notified', 'myReports', 'toursSeen', 'instanceGroups', 'profiles', 'lastAnnouncement', 'alphaUnlocked', 'instanceOrder', 'pinned', 'fonts', 'playlist', 'sharedFolders', 'shortcuts', 'railLinks', 'quickJoin', 'hiddenShards'];

async function applyProfile(profile) {
    const data = Object.fromEntries(Object.entries(profile.data ?? {}).filter(([key]) => !PROFILE_SKIP.includes(key) && (key in (store.settings ?? {}) || key in LOOK_DEFAULTS)));
    await persist(data);
    toast(`已套用設定方案「${profile.name}」`);
}

function renderProfiles() {
    const profiles = store.settings?.profiles ?? [];
    clear($('#profile-list')).append(...(profiles.length ? profiles.map((profile, i) => h('div', { class: 'media-row' },
        h('span', { class: 'media-name' }, icon('sliders'), profile.name, h('small', {}, `${Object.keys(profile.data ?? {}).length} 項設定`)),
        h('button', { type: 'button', class: 'mc primary', onclick: () => applyProfile(profile) }, '套用'),
        h('button', { type: 'button', class: 'mc', title: '用目前的設定覆蓋這個方案', onclick: async () => {
            if (!(await confirm(`要用目前的設定覆蓋「${profile.name}」嗎？`, '覆蓋'))) return;
            await persist({ profiles: profiles.map((p, j) => (j === i ? { name: p.name, data: profileSnapshot() } : p)) });
            toast(`已更新「${profile.name}」`);
        } }, '更新'),
        h('button', { type: 'button', class: 'mc icon-btn danger', title: '刪除', 'aria-label': `刪除「${profile.name}」`, onclick: async () => {
            if (!(await confirm(`要刪除設定方案「${profile.name}」嗎？目前的設定不會改變。`, '刪除'))) return;
            await persist({ profiles: profiles.filter((_, j) => j !== i) });
        } }, icon('trash')))) : [h('div', { class: 'hint' }, '還沒有設定方案。')]));
    $('#btn-profile-save').disabled = profiles.length >= 10;
}

const profileSnapshot = () => Object.fromEntries(Object.entries({ ...LOOK_DEFAULTS, ...(store.settings ?? {}) }).filter(([key]) => !PROFILE_SKIP.includes(key)));

async function renderScreens() {
    const screens = (await api.list_screens().catch(() => null)) ?? [];
    const select = $('#set-window-monitor');
    const current = store.settings?.windowMonitor ?? '';
    clear(select).append(h('option', { value: '' }, '自動（遊戲預設）'), ...screens.map((s, i) => h('option', { value: s.name }, `螢幕 ${i + 1}（${s.width}×${s.height}）`)));
    if (current && !screens.some((s) => s.name === current)) select.append(h('option', { value: current }, '之前選的螢幕（目前沒有接上）'));
    select.value = current;
    select.disabled = !screens.length;
}

function renderSync() {
    const groups = store.settings?.optionsSyncGroups ?? ['keys', 'video', 'audio', 'controls'];
    for (const box of $$('#sync-groups [data-sync]')) {
        box.checked = groups.includes(box.dataset.sync);
    }
    api.options_sync_count().then((n) => { $('#sync-state').textContent = n ? `已記下 ${n} 項選項` : '還沒有記下任何選項，玩一次遊戲或按左邊按鈕'; }, () => {});
}

const DISK_COLORS = ['#ffab19', '#4c97ff', '#59c059', '#9966ff', '#ff6680', '#40bf9f', '#cf63cf', '#ff8c1a', '#5cb1d6', '#8c8c8c', '#b3b300', '#e64d4d'];

async function scanDisk() {
    const box = clear($('#disk-usage'));
    box.append(h('div', { class: 'hint' }, '分析中，檔案多時需要幾秒⋯'));
    let usage;
    try {
        usage = await api.disk_usage();
    } catch (error) {
        clear(box);
        return fail(error);
    }
    $('#disk-total').textContent = `共 ${fmtBytes(usage.total)}`;
    const bar = (parts, total) => h('div', { class: 'disk-bar' }, ...parts.filter((p) => p.size > 0).map((p, i) => h('i', { style: `width:${(p.size / Math.max(1, total)) * 100}%;background:${DISK_COLORS[i % DISK_COLORS.length]}`, title: `${p.label} ${fmtBytes(p.size)}` })));
    const legend = (parts) => h('div', { class: 'disk-legend' }, ...parts.filter((p) => p.size > 0).map((p, i) => h('span', {}, h('i', { style: `background:${DISK_COLORS[i % DISK_COLORS.length]}` }), `${p.label} ${fmtBytes(p.size)}`)));
    const shared = [...usage.shared].sort((a, b) => b.size - a.size);
    const sharedTotal = shared.reduce((n, p) => n + p.size, 0);
    clear(box).append(
        usage.suggestions.length ? h('div', { class: 'disk-suggest' }, h('b', {}, '清理建議'), ...usage.suggestions.map((s) => h('div', { class: 'media-row' },
            h('span', { class: 'media-name' }, s.label, h('small', {}, s.detail)),
            h('b', { class: 'disk-size' }, fmtBytes(s.size)),
            h('button', { type: 'button', class: `mc${s.key === 'trash' ? ' danger' : ''}`, onclick: guard(async () => {
                if (!(await confirm(`${s.label}？\n${s.detail}\n可以釋放約 ${fmtBytes(s.size)}。`, '清理'))) return;
                try {
                    const freed = await api.clean_disk({ key: s.key });
                    toast(`已釋放 ${fmtBytes(freed)}`);
                    scanDisk();
                } catch (error) {
                    fail(error);
                }
            }) }, '清理')))) : h('div', { class: 'hint' }, '目前沒有需要清理的項目。'),
        h('div', { class: 'disk-group' }, h('b', {}, `啟動器共用檔案　${fmtBytes(sharedTotal)}`), bar(shared, sharedTotal), legend(shared)),
        ...usage.instances.map((inst) => h('details', { class: 'disk-group' },
            h('summary', {}, h('b', {}, inst.name), h('span', { class: 'disk-size' }, fmtBytes(inst.total)),
                h('div', { class: 'disk-bar mini' }, h('i', { style: `width:${(inst.total / Math.max(1, usage.instances[0]?.total ?? 1)) * 100}%;background:var(--accent)` }))),
            bar(inst.parts, inst.total), legend(inst.parts),
            h('div', { class: 'toolbar' }, h('button', { type: 'button', class: 'mc', onclick: () => api.open_instance_folder({ id: inst.id, sub: '' }).then((path) => api.open_path({ path })).catch(fail) }, icon('folder'), '開啟資料夾')))));
}

const COLOR_LABELS = [
    ['content-bg', '內容背景'], ['content-bg-alt', '次要背景'], ['content-bg-sunken', '凹陷區塊'], ['panel-face', '面板與卡片'],
    ['text', '文字'], ['text-subtle', '次要文字'], ['border', '邊框'], ['border-soft', '淡邊框'],
    ['chrome-bg', '導航列與側欄背景'], ['chrome-ink', '導航列與側欄文字'], ['glass-bg', '主頁玻璃面板'], ['glass-ink', '主頁玻璃面板文字']
];

function cssHex(value) {
    const ctx = document.createElement('canvas').getContext('2d');
    ctx.fillStyle = '#808080';
    ctx.fillStyle = value.trim() || '#808080';
    const out = ctx.fillStyle;
    if (out.startsWith('#')) return out;
    const [r, g, b] = (out.match(/\d+(\.\d+)?/g) ?? [128, 128, 128]).map(Number);
    return `#${[r, g, b].map((n) => Math.round(n).toString(16).padStart(2, '0')).join('')}`;
}

function renderColorEditor() {
    const mode = document.documentElement.dataset.mode || 'dark';
    const overrides = store.settings?.colorOverrides ?? {};
    $('#color-editor-mode').textContent = `正在編輯${mode === 'dark' ? '深色' : '亮色'}主題的配色；切換亮暗後可以另外設定另一組。`;
    clear($('#color-editor')).append(...COLOR_LABELS.map(([token, label]) => {
        const key = `${mode}.${token}`;
        const custom = overrides[key];
        const current = custom || cssHex(getComputedStyle(document.documentElement).getPropertyValue(`--${token}`));
        return h('div', { class: 'color-field' },
            h('button', {
                type: 'button',
                class: 'swatch',
                style: `--swatch:${current}`,
                'aria-label': `調整「${label}」`,
                onclick: (e) => pickColor(e.currentTarget, current, (value) => persist({ colorOverrides: { ...(store.settings?.colorOverrides ?? {}), [key]: value } }), label)
            }),
            h('span', {}, label, h('small', {}, custom ? custom : '預設')),
            custom ? h('button', { type: 'button', class: 'mc icon-btn reset-btn', title: '還原成預設顏色', 'aria-label': `把「${label}」還原成預設顏色`, onclick: () => {
                const next = { ...(store.settings?.colorOverrides ?? {}) };
                delete next[key];
                persist({ colorOverrides: next });
            } }, icon('reload')) : null);
    }));
}

function renderCountdownPick() {
    const at = store.settings?.countdownAt ? new Date(store.settings.countdownAt) : null;
    const now = new Date();
    const y = at?.getFullYear() ?? 0;
    const m = at ? at.getMonth() + 1 : now.getMonth() + 1;
    const days = new Date(y || now.getFullYear(), m, 0).getDate();
    const opts = (from, to, pad, unit, current, step = 1) => {
        const list = [];
        for (let n = from; n <= to; n += step) list.push(h('option', { value: String(n), selected: n === current }, `${pad ? String(n).padStart(2, '0') : n}${unit}`));
        return list;
    };
    const [ys, ms, ds, hs, is] = $$('#set-countdown-at select');
    clear(ys).append(h('option', { value: '0', selected: !y }, '未設定'), ...opts(now.getFullYear(), now.getFullYear() + 5, false, ' 年', y));
    clear(ms).append(...opts(1, 12, false, ' 月', m));
    clear(ds).append(...opts(1, days, false, ' 日', at ? at.getDate() : now.getDate()));
    clear(hs).append(...opts(0, 23, true, ' 時', at ? at.getHours() : 20));
    clear(is).append(...opts(0, 55, true, ' 分', at ? at.getMinutes() - (at.getMinutes() % 5) : 0, 5));
    for (const s of [ms, ds, hs, is]) s.disabled = !y;
    $('#btn-countdown-clear').disabled = !y;
}

function renderColorFields() {
    for (const field of $$('.color-field[data-color-key]')) {
        const key = field.dataset.colorKey;
        const value = store.settings?.[key] ?? '';
        const label = field.dataset.colorLabel;
        const swatch = h('button', {
            type: 'button',
            class: `swatch${value ? '' : ' empty'}`,
            style: value ? `--swatch:${value}` : '',
            title: `選擇${label}`,
            'aria-label': `選擇${label}`,
            onclick: (e) => pickColor(e.currentTarget, value || '#8491ff', (v) => persist({ [key]: v }), label)
        });
        clear(field).append(swatch, h('span', {}, label, h('small', {}, value || '預設')),
            value ? h('button', { type: 'button', class: 'mc icon-btn reset-btn', title: '還原成預設顏色', 'aria-label': `把「${label}」還原成預設顏色`, onclick: () => persist({ [key]: '' }) }, icon('reload')) : null);
    }
}

function renderShardVisibility() {
    const hidden = store.settings?.hiddenShards ?? [];
    clear($('#shard-visibility')).append(...SHARDS.map((shard) => h('label', {},
        h('input', {
            type: 'checkbox',
            checked: !hidden.includes(shard.host),
            onchange: (e) => persist({ hiddenShards: e.target.checked ? hidden.filter((x) => x !== shard.host) : [...hidden, shard.host] })
        }), shard.label)));
}

const RAIL_LOCKED = ['home', 'instances', 'settings'];

export function railItems() {
    const links = $$('.rail a[data-view]').filter((a) => a.dataset.view !== 'about');
    const order = store.settings?.railOrder ?? [];
    const rank = (view) => (order.includes(view) ? order.indexOf(view) : order.length + links.findIndex((a) => a.dataset.view === view));
    return links.map((a) => a.dataset.view).sort((a, b) => rank(a) - rank(b));
}

function renderRailEditor() {
    const items = railItems();
    const hidden = store.settings?.railHidden ?? [];
    const move = (i, step) => {
        const next = [...items];
        [next[i], next[i + step]] = [next[i + step], next[i]];
        persist({ railOrder: next });
    };
    clear($('#rail-editor')).append(...items.map((view, i) => {
        const link = $(`.rail a[data-view="${view}"]`);
        return h('div', { class: 'rail-item' },
            h('label', {},
                h('input', {
                    type: 'checkbox',
                    checked: !hidden.includes(view),
                    disabled: RAIL_LOCKED.includes(view),
                    onchange: (e) => persist({ railHidden: e.target.checked ? hidden.filter((x) => x !== view) : [...hidden, view] })
                }),
                link.querySelector('svg')?.cloneNode(true), link.title),
            h('button', { type: 'button', class: 'mc icon-btn', title: '往上移', 'aria-label': `${link.title}往上移`, disabled: i === 0, onclick: () => move(i, -1) }, icon('arrow-up')),
            h('button', { type: 'button', class: 'mc icon-btn', title: '往下移', 'aria-label': `${link.title}往下移`, disabled: i === items.length - 1, onclick: () => move(i, 1) }, icon('arrow-down')));
    }));
}

let heroSlides = null;

async function renderBgPins() {
    const box = $('#bg-pins');
    heroSlides ??= await api.site_json({ path: '/api/launcher/hero' }).then((d) => d?.slides ?? [], () => []);
    const pin = store.settings?.bgPin ?? '';
    clear(box).append(
        h('button', { type: 'button', class: 'bg-pin random', 'aria-pressed': String(!pin), onclick: () => persist({ bgPin: '' }) }, icon('reload'), '隨機輪播'),
        ...heroSlides.map((slide) => h('button', {
            type: 'button',
            class: 'bg-pin',
            title: [slide.description, slide.author && `攝影：${slide.author}`].filter(Boolean).join('｜'),
            'aria-label': slide.description || '背景圖片',
            'aria-pressed': String(pin === slide.image),
            style: `background-image:url("${slide.image.replace(/&w=\d+$/, '&w=320')}")`,
            onclick: () => persist({ bgPin: slide.image })
        })));
}

function syncConditions() {
    const settings = store.settings ?? {};
    for (const el of $$('#view-settings [data-when]')) {
        const [key, want] = el.dataset.when.split('=');
        const value = settings[key] ?? LOOK_DEFAULTS[key];
        el.style.display = (want === undefined ? !!value : String(value) === want) ? '' : 'none';
    }
}

export const JVM_PRESETS = [
    ['G1 調校（預設）', '平衡穩定，適合大多數電腦', '-XX:+UnlockExperimentalVMOptions -XX:+UseG1GC -XX:G1NewSizePercent=20 -XX:G1ReservePercent=20 -XX:MaxGCPauseMillis=50 -XX:G1HeapRegionSize=32M -XX:-UseAdaptiveSizePolicy -XX:-OmitStackTraceInFastThrow'],
    ["Aikar's Flags", '社群常用的 G1 調校，記憶體 6 GB 以上效果較好', '-XX:+UseG1GC -XX:+ParallelRefProcEnabled -XX:MaxGCPauseMillis=200 -XX:+UnlockExperimentalVMOptions -XX:+DisableExplicitGC -XX:G1NewSizePercent=30 -XX:G1MaxNewSizePercent=40 -XX:G1HeapRegionSize=8M -XX:G1ReservePercent=20 -XX:G1HeapWastePercent=5 -XX:G1MixedGCCountTarget=4 -XX:InitiatingHeapOccupancyPercent=15 -XX:G1MixedGCLiveThresholdPercent=90 -XX:G1RSetUpdatingPauseTimePercent=5 -XX:SurvivorRatio=32 -XX:+PerfDisableSharedMem -XX:MaxTenuringThreshold=1'],
    ['分代式 ZGC', '幾乎沒有 GC 停頓，需要 Java 21 以上，記憶體 8 GB 以上較適合（Java 23 起分代模式是預設，啟動器會自動略過已移除的參數）', '-XX:+UseZGC -XX:+ZGenerational -XX:-OmitStackTraceInFastThrow']
];

export function jvmPresetButtons(target, onPick) {
    return JVM_PRESETS.map(([label, hint, args]) => h('button', {
        type: 'button',
        class: 'mc',
        title: hint,
        onclick: () => {
            target.value = args;
            onPick();
        }
    }, label));
}

const PROXY_PATTERN = /^https?:\/\/([^\s@/]+@)?[A-Za-z0-9.\-[\]:_]+:\d{1,5}\/?$/;
export const SHARE_LABELS = [['saves', '存檔'], ['resourcepacks', '資源包'], ['shaderpacks', '光影'], ['screenshots', '截圖']];
export const SHARE_INFO = {
    saves: ['map', '在任一實例建立或下載的單人世界，其他實例也看得到。只建議同一個 Minecraft 版本的實例共用，高版本開過的世界會被升級。'],
    resourcepacks: ['image', '在任一實例加入的資源包，其他實例也看得到；啟用與排序仍由各實例自己決定。'],
    shaderpacks: ['eye', '光影包只要放一份，所有參與的實例都能選用。'],
    screenshots: ['image', '所有參與實例的截圖集中在同一個資料夾，相簿也會一起顯示。']
};

function renderSharedSummary() {
    const box = $('#set-shared');
    if (!box) return;
    const groups = (store.settings?.instanceGroups ?? []).filter((g) => g.shared?.length);
    clear(box).append(
        h('span', { class: 'share-summary' }, groups.length
            ? groups.map((g) => `「${g.name}」共用 ${g.shared.map((f) => SHARE_LABELS.find(([v]) => v === f)?.[1]).filter(Boolean).join('、')}`).join('；')
            : '共用資料夾改在實例分組中設定：每個分組可以各自選擇要共用的資料夾。'),
        h('button', { type: 'button', class: 'mc', onclick: () => { location.hash = '#instances'; } }, icon('folder'), '前往實例分組'));
}

const ACCENTS = [
    ['', '官網配色'],
    ['#2ECC71', '翡翠綠'],
    ['#4dabf7', '天空藍'],
    ['#b197fc', '紫水晶'],
    ['#f783ac', '櫻花粉'],
    ['#ff6b6b', '紅石'],
    ['#ffa94d', '橘子'],
    ['#fcc419', '金塊']
];

const LABELS = {
    '#set-memory': (v) => v,
    '#set-slide': (v) => (!Number(v) ? '不輪播' : v >= 60 ? `${Math.floor(v / 60)} 分${v % 60 ? ` ${v % 60} 秒` : '鐘'}` : `${v} 秒`),
    '#set-scale': (v) => `${v}%`,
    '#set-threads': (v) => `${v} 個`,
    '#set-volume': (v) => (Number(v) ? `${v}%` : '靜音'),
    '#set-panel': (v) => `${v}%`,
    '#set-dim': (v) => (Number(v) ? `${v}%` : '不暗化'),
    '#set-blur': (v) => (Number(v) ? `${v}` : '不模糊'),
    '#set-font-scale': (v) => `${v}%`,
    '#set-cover-shade': (v) => (Number(v) ? `${v}%` : '不加遮罩'),
    '#set-click-volume': (v) => (Number(v) ? `${v}%` : '靜音'),
    '#set-toast-seconds': (v) => `${v} 秒`
};

let javaReady = false;

async function fillJavaOptions(current) {
    const select = clear($('#set-java'));
    select.append(h('option', { value: '' }, '自動（推薦）'));
    let list = [];
    try {
        list = await api.list_java();
    } catch (error) {
        fail(error);
    }
    for (const java of list) {
        select.append(h('option', { value: java.path },
            `Java ${java.version}${java.source === 'mojang' ? '（啟動器下載）' : ''} — ${java.path}`));
    }
    if (current && !list.some((j) => j.path === current)) {
        select.append(h('option', { value: current }, `${current}（目前設定）`));
    }
    select.value = current ?? '';
    javaReady = true;
}

function syncLabels() {
    for (const [sel, fmt] of Object.entries(LABELS)) {
        const el = $(sel);
        $(`${sel}-value`).textContent = fmt(el.value);
        rangeFill(el);
    }
}

function read() {
    const patch = {};
    for (const [key, sel] of Object.entries(FIELDS)) {
        const el = $(sel);
        if (key === 'javaPath' && !javaReady) continue;
        if (key === 'proxy' && el.value.trim() && !PROXY_PATTERN.test(el.value.trim())) continue;
        if (el.type === 'checkbox') patch[key] = el.checked;
        else if (el.type === 'number' || el.type === 'range') patch[key] = clampNumber(el);
        else if (el.classList.contains('hour-select')) patch[key] = Number(el.value);
        else patch[key] = el.value;
    }
    patch.backupKeep = Math.max(1, patch.backupKeep);
    return patch;
}

function renderAccent() {
    const current = (store.settings?.accent ?? '').toLowerCase();
    const custom = current && !ACCENTS.some(([value]) => value.toLowerCase() === current);
    const swatch = ([value, label]) => h('button', {
        type: 'button',
        class: `swatch${value ? '' : ' site'}`,
        style: value ? `--swatch:${value}` : '',
        title: label,
        'aria-label': label,
        'aria-pressed': String(value.toLowerCase() === current),
        onclick: () => persist({ accent: value })
    });
    clear($('#set-accent')).append(
        ...ACCENTS.map(swatch),
        h('button', {
            type: 'button',
            class: 'swatch custom',
            title: '自訂顏色',
            'aria-label': '自訂顏色',
            'aria-pressed': String(!!custom),
            'aria-haspopup': 'dialog',
            style: custom ? `--swatch:${current}` : '',
            onclick: (e) => pickColor(e.currentTarget, custom ? current : '#8491ff', (value) => persist({ accent: value }), '自訂主題色')
        })
    );
}

function write() {
    const settings = store.settings;
    if (!settings) return;
    for (const [key, sel] of Object.entries(FIELDS)) {
        const el = $(sel);
        if (el === $('#set-java') || el === document.activeElement) continue;
        const value = settings[key] ?? LOOK_DEFAULTS[key];
        if (el.type === 'checkbox') el.checked = !!value;
        else el.value = value ?? '';
        if (el.tagName === 'SELECT' && el.selectedIndex < 0) el.selectedIndex = 0;
    }
    $('#set-backup-keep').disabled = !settings.backupOnLaunch;
    renderSharedSummary();
    $('#bg-state').textContent = !settings.background ? '目前使用官網輪播' : /\.(mp4|webm)$/i.test(settings.background) ? '目前使用自訂影片' : '目前使用自訂圖片';
    $('#set-font option[value="custom"]').disabled = !settings.customFont;
    renderMedia();
    renderShortcuts();
    renderRailLinks();
    renderProfiles();
    renderSync();
    renderColorEditor();
    renderCountdownPick();
    $('#btn-forget-window').disabled = !settings.rememberWindow;
    $('#set-slide').disabled = !!settings.background || !!settings.bgPin;
    $('#set-account-name').checked = !settings.hideAccountName;
    for (const box of $$('#tool-visibility [data-tool]')) box.checked = !(settings.hiddenTools ?? []).includes(box.dataset.tool);
    syncLabels();
    syncConditions();
    renderAccent();
    renderColorFields();
    renderShardVisibility();
    renderRailEditor();
    if (heroSlides) renderBgPins();
}

let savedTimer = 0;
async function persist(patch) {
    const state = $('#save-state');
    try {
        await saveSettings(patch);
        store.versions = null;
        state.textContent = '已儲存';
        state.classList.add('saved');
        clearTimeout(savedTimer);
        savedTimer = setTimeout(() => {
            state.textContent = '變更會自動儲存';
            state.classList.remove('saved');
        }, 1500);
    } catch (error) {
        fail(error);
    }
}

let manualCheck = false;

function renderUpdate() {
    const box = clear($('#update-status'));
    const u = store.update;
    if (!u) {
        box.append(h('div', { class: 'empty-state' }, store.settings?.launcherUpdate === 'off' ? '已關閉自動檢查，按「立即檢查更新」查看有沒有新版。' : '正在檢查更新⋯'));
        return;
    }
    const row = (label, value) => h('div', { class: 'row' }, h('div', { class: 'grow' }, h('div', { class: 'name' }, label), h('div', { class: 'meta' }, value)));
    const latest = u.latest ? `v${u.latest.version}${u.newer ? '（可以更新）' : '（已是最新）'}` : u.reachable ? '—' : '暫時連不上官網';
    const signed = !u.latest ? '—' : !u.self ? '這個平台請到官網下載新版' : u.signed ? '已附輝煌官方數位簽章，安裝前會先驗證' : '這個版本沒有數位簽章，啟動器不會自動安裝';
    box.append(row('目前版本', `v${u.current}`), row('最新版本', latest), row('數位簽章', signed),
        row('上次檢查', new Date(u.checkedAt).toLocaleString('zh-TW', { hour12: false })));
    if (manualCheck) {
        manualCheck = false;
        toast(!u.reachable ? '暫時連不上官網，請稍後再試' : u.newer ? `有新版 v${u.latest.version}，可以點右上角的更新按鈕` : '已經是最新版本', u.reachable ? '' : 'warn');
    }
}

const PAGE_KEY = 'brilliant.settings.page';
const GROUP_KEY = 'brilliant.settings.lookGroup';

function initPages(view) {
    const pages = [...view.querySelectorAll(':scope > h2')].map((title) => {
        const page = h('section', { class: 'settings-page', 'data-page': title.id });
        title.before(page);
        for (let el = title; el && (el === title || el.nodeName !== 'H2') && el.id !== 'settings-empty';) {
            const next = el.nextElementSibling;
            page.append(el);
            el = next;
        }
        return page;
    });
    const links = [...view.querySelectorAll('#settings-nav [data-jump]')];
    const search = $('#settings-search');
    const lookPage = pages.find((p) => p.dataset.page === 'set-look');
    const groupNames = ['風格'];
    for (const block of [...lookPage.children].slice(1)) {
        if (block.matches('h3.look-group')) groupNames.push(block.textContent);
        block.dataset.group = String(groupNames.length - 1);
    }
    let group = '0';
    try {
        group = localStorage.getItem(GROUP_KEY) || '0';
    } catch {}
    if (!groupNames[group]) group = '0';
    const groupTabs = h('div', { class: 'segmented look-tabs', role: 'tablist', 'aria-label': '外觀分類' },
        ...groupNames.map((name, i) => h('button', {
            type: 'button',
            role: 'tab',
            'data-group': String(i),
            onclick: () => {
                group = String(i);
                try {
                    localStorage.setItem(GROUP_KEY, group);
                } catch {}
                render();
                $('.page-wrap').scrollTop = 0;
            }
        }, name)));
    lookPage.querySelector('h2').after(groupTabs);
    let current = '';
    try {
        current = localStorage.getItem(PAGE_KEY) || '';
    } catch {}
    const render = () => {
        const words = search.value.trim().toLowerCase().split(/\s+/).filter(Boolean);
        view.classList.toggle('searching', words.length > 0);
        let found = 0;
        for (const page of pages) {
            const title = page.querySelector('h2').textContent.toLowerCase();
            let hit = false;
            for (const block of page.children) {
                if (block.nodeName === 'H2' || block === groupTabs) continue;
                const text = `${title} ${block.textContent.toLowerCase()}`;
                const match = words.every((w) => text.includes(w));
                block.classList.toggle('search-miss', !match);
                block.classList.toggle('group-off', !words.length && block.dataset.group !== undefined && (block.dataset.group !== group || block.matches('h3.look-group')));
                hit ||= match;
            }
            page.hidden = words.length ? !hit : page.dataset.page !== current;
            if (words.length && hit) found++;
        }
        for (const link of links) link.setAttribute('aria-current', String(!words.length && link.dataset.jump === current));
        groupTabs.hidden = words.length > 0;
        for (const tab of groupTabs.children) tab.setAttribute('aria-selected', String(tab.dataset.group === group));
        $('#settings-empty').hidden = !words.length || found > 0;
    };
    const show = (id) => {
        current = pages.some((p) => p.dataset.page === id) ? id : pages[0].dataset.page;
        try {
            localStorage.setItem(PAGE_KEY, current);
        } catch {}
        search.value = '';
        render();
        view.scrollTop = 0;
    };
    $('#settings-nav').addEventListener('click', (e) => {
        const link = e.target.closest('[data-jump]');
        if (!link) return;
        e.preventDefault();
        show(link.dataset.jump);
    });
    search.addEventListener('input', render);
    search.addEventListener('keydown', (e) => {
        if (e.key === 'Escape' && search.value) {
            e.stopPropagation();
            search.value = '';
            render();
        }
    });
    show(current);
}

export function initSettings() {
    on('update-status', renderUpdate);
    const syncAlpha = () => { $('#opt-alpha').hidden = !store.settings?.alphaUnlocked && store.settings?.updateChannel !== 'alpha'; };
    on('settings', syncAlpha);
    syncAlpha();
    let lastChannel = store.settings?.updateChannel || 'stable';
    on('settings', () => {
        const channel = store.settings?.updateChannel || 'stable';
        if (channel === lastChannel) return;
        lastChannel = channel;
        toast(channel === 'stable' ? '已切到正式版通道，目前的版本會用到下一個正式版推出為止' : `已切到 ${channel === 'beta' ? 'Beta 搶先版' : 'Alpha 測試版'}通道，正在檢查更新`);
        emit('check-update');
    });
    on('settings', () => { if (!store.update) renderUpdate(); });
    $('#btn-check-update').addEventListener('click', () => {
        manualCheck = true;
        emit('check-update');
    });
    $('#btn-update-page').addEventListener('click', () => openUrl('https://www.brilliantw.net/launcher/'));
    const view = $('#view-settings');
    const isSetting = (el) => Object.values(FIELDS).some((sel) => el.matches(sel));
    let typing = 0;
    view.addEventListener('input', (e) => {
        if (!isSetting(e.target)) return;
        syncLabels();
        clearTimeout(typing);
        typing = setTimeout(() => persist(read()), e.target.type === 'range' ? 250 : 600);
    });
    view.addEventListener('change', (e) => {
        if (!isSetting(e.target)) return;
        clearTimeout(typing);
        persist(read());
    });

    for (const select of $$('.hour-select')) select.append(...Array.from({ length: 24 }, (_, hour) => h('option', { value: String(hour) }, `${String(hour).padStart(2, '0')}:00`)));
    initPages(view);
    renderLookPresets();
    renderCommunityLooks();
    renderBgPins();
    $('#btn-look-reset').addEventListener('click', guard(async () => {
        if (!(await confirm('要把主題色、按鈕風格、邊角、字體、側欄、動畫等外觀設定全部還原成預設嗎？\n背景圖片與音樂不會被移除。', '還原'))) return;
        await persist(LOOK_DEFAULTS);
        toast('已還原預設外觀');
    }));
    $('#btn-look-export').addEventListener('click', exportLook);
    provide('revealSetting', revealSetting);
    provide('applyProfile', applyProfile);
    $('#btn-global-shortcut').addEventListener('click', async (e) => {
        const next = await captureCombo(e.currentTarget);
        if (next === null) return;
        if (next && !/^(Ctrl|Alt)\+/.test(next)) return toast('系統層級快捷鍵至少要包含 Ctrl 或 Alt，避免平常打字時誤觸', 'warn');
        await persist({ globalShortcut: next });
        renderShortcuts();
    });
    $('#btn-shortcuts-reset').addEventListener('click', guard(async () => {
        if (!(await confirm('要把所有啟動器內的快捷鍵還原成預設嗎？', '還原'))) return;
        await persist({ shortcuts: {} });
    }));
    $('#btn-rail-link-folder').addEventListener('click', guard(async () => {
        const folder = await window.__TAURI__.dialog.open({ directory: true }).catch(() => null);
        if (folder) $('#rail-link-target').value = folder;
    }));
    $('#btn-rail-link-add').addEventListener('click', guard(async () => {
        const name = $('#rail-link-name').value.trim();
        const target = $('#rail-link-target').value.trim();
        if (!name || !target) return toast('請填寫名稱和網址（或資料夾）', 'warn');
        if (/^[a-z][a-z0-9+.-]*:\/\//i.test(target) && !/^https?:\/\//i.test(target)) return toast('網址只支援 http:// 或 https:// 開頭', 'warn');
        await persist({ railLinks: [...(store.settings?.railLinks ?? []), { name, target }] });
        $('#rail-link-name').value = '';
        $('#rail-link-target').value = '';
    }));
    renderScreens();
    $('#sync-groups').addEventListener('change', (e) => {
        const groups = $$('#sync-groups [data-sync]').filter((b) => b.checked).map((b) => b.dataset.sync);
        persist({ optionsSyncGroups: groups });
    });
    $('#btn-sync-capture').addEventListener('click', guard(async () => {
        const instance = store.instances.find((i) => i.id === store.settings?.selectedInstance) ?? store.instances[0];
        if (!instance) return toast('還沒有任何實例', 'warn');
        try {
            const n = await api.options_sync_capture({ id: instance.id });
            toast(n ? `已從「${instance.name}」記下 ${n} 項選項，下次開其他實例會自動套用` : `「${instance.name}」還沒有遊戲選項檔，先玩一次再試`);
            renderSync();
        } catch (error) {
            fail(error);
        }
    }));
    $('#btn-forget-window').addEventListener('click', guard(async () => {
        await api.forget_window().catch(fail);
        toast('已忘記記住的遊戲視窗位置');
    }));
    $('#btn-disk-scan').addEventListener('click', guard(scanDisk));
    $('#btn-home-edit').addEventListener('click', () => $('#home-edit-toggle').click());
    let language = store.settings?.language ?? 'zh-TW';
    on('settings', () => {
        const next = store.settings?.language ?? 'zh-TW';
        if (next === language) return;
        language = next;
        setTimeout(() => location.reload(), 300);
    });
    $('#set-countdown-at').addEventListener('change', () => {
        const v = Object.fromEntries($$('#set-countdown-at select').map((s) => [s.dataset.part, Number(s.value)]));
        const days = new Date(v.y, v.m, 0).getDate();
        persist({ countdownAt: v.y ? new Date(v.y, v.m - 1, Math.min(v.d, days), v.h, v.i).getTime() : 0 });
    });
    $('#btn-countdown-clear').addEventListener('click', () => persist({ countdownAt: 0 }));
    $('#btn-colors-reset').addEventListener('click', guard(async () => {
        const mode = document.documentElement.dataset.mode || 'dark';
        if (!(await confirm(`要把${mode === 'dark' ? '深色' : '亮色'}主題的進階配色全部還原嗎？`, '還原'))) return;
        await persist({ colorOverrides: Object.fromEntries(Object.entries(store.settings?.colorOverrides ?? {}).filter(([key]) => !key.startsWith(`${mode}.`))) });
    }));
    $('#btn-profile-save').addEventListener('click', guard(async () => {
        const name = $('#profile-name').value.trim();
        if (!name) return toast('請輸入方案名稱', 'warn');
        const profiles = store.settings?.profiles ?? [];
        if (profiles.some((p) => p.name === name)) return toast(`已經有叫「${name}」的方案了，要更新請按該方案的「更新」`, 'warn');
        await persist({ profiles: [...profiles, { name, data: profileSnapshot() }] });
        $('#profile-name').value = '';
        toast(`已存成方案「${name}」`);
    }));
    $('#btn-look-import').addEventListener('click', guard(importLook));
    $('#set-account-name').addEventListener('change', (e) => persist({ hideAccountName: !e.target.checked }));
    $('#tool-visibility').addEventListener('change', (e) => {
        const tool = e.target.dataset.tool;
        if (!tool) return;
        const hidden = store.settings?.hiddenTools ?? [];
        persist({ hiddenTools: e.target.checked ? hidden.filter((x) => x !== tool) : [...hidden, tool] });
    });
    for (const sel of ['#set-click-type', '#set-click-volume']) {
        $(sel).addEventListener('change', () => playClick($('#set-click-type').value, $('#set-click-volume').value));
    }

    const setMedia = (kind) => async (path) => {
        if (!path) return;
        try {
            await persist({ [kind]: await api.set_media({ kind, path }) });
        } catch (error) {
            fail(error);
        }
    };
    $('#btn-bg-pick').addEventListener('click', guard(async () => setMedia('background')(await pickFile('background').catch(fail))));
    const addMusic = async (paths) => {
        const added = await addMedia('music', paths);
        if (!added.length) return;
        const playlist = [...(store.settings?.playlist ?? []), ...added];
        await persist({ playlist, music: store.settings?.music || added[0].path });
        toast(`已加入 ${added.length} 首音樂`);
    };
    $('#btn-music-pick').addEventListener('click', guard(async () => addMusic(await pickFile('music', { multiple: true }).catch(fail))));
    $('#btn-music-next').addEventListener('click', () => nextTrack());
    on('music', renderMedia);
    acceptFiles($('#btn-bg-pick').parentElement, 'background', setMedia('background'));
    acceptFiles($('#btn-music-pick').parentElement, 'music', addMusic, { multiple: true });
    const addFonts = async (paths) => {
        const added = await addMedia('font', paths);
        if (!added.length) return;
        await persist({ fonts: [...(store.settings?.fonts ?? []), ...added], font: 'custom', customFont: added.at(-1).path });
        toast(added.length > 1 ? `已加入 ${added.length} 個字型，目前使用「${added.at(-1).name}」` : `已套用字型「${added[0].name}」`);
    };
    $('#btn-font-pick').addEventListener('click', guard(async () => addFonts(await pickFile('font', { multiple: true }).catch(fail))));
    acceptFiles($('#btn-font-pick').parentElement, 'font', addFonts, { multiple: true });
    $('#btn-bg-reset').addEventListener('click', guard(() => persist({ background: '' })));

    $('#jvm-presets').append(...jvmPresetButtons($('#set-jvm'), () => {
        persist(read());
        toast('已套用 JVM 參數預設組');
    }));
    on('instances', renderSharedSummary);
    $('#btn-config-export').addEventListener('click', guard(async () => {
        try {
            const path = await api.export_config();
            toast(`已匯出設定到「下載」資料夾：${path.split(/[\\/]/).pop()}`);
            api.open_path({ path: path.replace(/[\\/][^\\/]+$/, '') }).catch(() => {});
        } catch (error) {
            fail(error);
        }
    }));
    const importConfig = async (path) => {
        if (!path) return;
        if (!(await confirm('匯入會用檔案裡的啟動器設定取代目前的設定，並加入目前沒有的實例（已存在的實例不會被動到）。要繼續嗎？', '匯入'))) return;
        try {
            const result = await api.import_config({ path });
            await loadSettings();
            emit('settings');
            await loadInstances().catch(() => {});
            toast(`匯入完成：${result.settings ? '已套用啟動器設定，' : ''}新增 ${result.added} 個實例${result.skipped ? `，略過 ${result.skipped} 個已存在的實例` : ''}`);
            for (const note of result.notes ?? []) toast(note, 'warn');
        } catch (error) {
            fail(error);
        }
    };
    $('#btn-config-import').addEventListener('click', guard(async () => importConfig(await pickFile('config').catch(fail))));
    acceptFiles($('#btn-config-import'), 'config', importConfig);

    $('#btn-open-root').addEventListener('click', guard(async () => {
        const paths = await api.launcher_paths();
        await api.open_path({ path: paths[0][1] }).catch(fail);
    }));
    $('#btn-clean-cache').addEventListener('click', guard(async () => {
        if (!(await confirm('要清理快取嗎？存檔、模組與設定都不會受影響；回收資料夾裡放超過 30 天的項目會永久刪除。', '清理'))) return;
        try {
            toast(`已清理 ${fmtBytes(await api.clean_cache())}`);
        } catch (error) {
            fail(error);
        }
    }));

    api.system_info().then((info) => {
        const gpu = gpuName();
        const integrated = isIntegratedGpu(gpu);
        clear($('#hardware')).append(
            h('div', { class: 'grow' },
                h('b', {}, '你的電腦'),
                h('small', {}, `記憶體 ${Math.round(info.memoryMb / 1024)} GB｜${info.cpus} 執行緒｜${gpu || '顯示卡未知'}`),
                h('small', {}, `建議遊戲記憶體 ${info.recommendedMb} MB${integrated ? '；內建顯示卡建議安裝「效能優化包」，不建議開光影' : ''}`)),
            h('button', {
                type: 'button',
                class: 'mc',
                onclick: () => persist({ memoryMb: info.recommendedMb })
            }, '套用建議'));
    }).catch(() => {});

    on('settings', write);
    write();
    fillJavaOptions(store.settings?.javaPath);

    api.launcher_paths().then((paths) => {
        clear($('#about-paths')).append(
            ...paths.map(([label, path]) =>
                h('div', { class: 'row' },
                    h('div', { class: 'grow' },
                        h('div', { class: 'name' }, label),
                        h('div', { class: 'meta' }, path)),
                    h('button', {
                        type: 'button',
                        class: 'mc',
                        onclick: () => api.open_path({ path }).catch(fail)
                    }, '開啟'))
            )
        );
    }).catch(() => {});
}
