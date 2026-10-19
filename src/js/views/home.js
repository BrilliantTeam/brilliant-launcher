import { api, listen } from '../api.js';
import { $, clear, dialog, fail, fmtBytes, h, toast } from '../ui.js';
import { avatarUrl, emit, isRunning, loadInstances, on, selectedAccount, selectedInstance, store, saveSettings } from '../store.js';
import { initNews } from './news.js';
import { icon, instanceIconName, instanceVisual } from '../icons.js';
import { addOffline, loginMicrosoft } from './accounts.js';
import { quickCreate, upgradeFlow, versionData } from './instances.js';
import { ready, upgradeTarget } from '../services.js';
import { clearRecentLog, recentLog } from './logview.js';
import { diagnose } from '../crash.js';

export const SHARDS = [
    { host: 's1.brilliantw.net', label: '炒飯服' },
    { host: 's2.brilliantw.net', label: '書碼服' },
    { host: 's3.brilliantw.net', label: '羽森服' },
    { host: 's4.brilliantw.net', label: '恐龍服' }
];

const PING_INTERVAL_MS = 60_000;
let busy = false;
let statuses = [];

const shardOf = (host) => SHARDS.find((shard) => shard.host === host) ?? null;

function renderAccountChip() {
    const account = selectedAccount();
    $('#chip-avatar').src = avatarUrl(account);
    $('#chip-name').textContent = account ? account.name : '登入帳號';
}

function renderPlayBar() {
    const instance = selectedInstance();
    const account = selectedAccount();
    const running = instance && isRunning(instance.id);
    const target = shardOf(store.settings?.quickJoin);
    const button = $('#btn-play');

    clear($('#home-instance')).append(...(instance ? [instanceVisual(instance), instance.name] : ['尚未建立實例']));
    $('#home-meta').textContent = !instance
        ? '先到「實例」建立一個遊戲實例'
        : !account
            ? '尚未登入帳號'
            : `${instance.mcVersion}　${instance.loader === 'fabric' ? 'Fabric' : '原版'}　${account.name}${ready.has(instance.id) && !running ? '　已就緒' : ''}`;
    renderUpgrade(instance);
    document.body.classList.toggle('launching', busy);

    clear(button);
    if (running) button.append('結束遊戲');
    else if (busy) button.append('準備中⋯');
    else if (target) button.append('開始遊戲', h('small', {}, `直連 ${target.label}`));
    else button.append('開始遊戲', h('small', {}, store.settings?.quickJoin ? `連線 ${store.settings.quickJoin}` : '進入主畫面'));
    button.classList.toggle('stop', !!running);
    button.classList.toggle('busy', busy && !running);
    button.disabled = !instance || !account || (busy && !running);
    renderAccountChip();
    renderSetup(account, instance);
}

let creating = false;

const dismissed = new Set();

function renderUpgrade(instance) {
    const box = $('#upgrade-banner');
    const target = upgradeTarget(instance);
    box.hidden = !target || dismissed.has(`${instance?.id}:${target}`) || busy;
    if (box.hidden) return;
    clear(box).append(
        h('div', { class: 'grow' },
            h('b', {}, `伺服器已更新到 Minecraft ${target}`),
            h('small', {}, `「${instance.name}」還在 ${instance.mcVersion}，一鍵升級並自動更新模組。`)),
        h('button', { type: 'button', class: 'mc primary', onclick: () => upgradeFlow(instance, target) }, '一鍵升級'),
        h('button', { type: 'button', class: 'mc icon-btn', title: '稍後', 'aria-label': '稍後', onclick: () => { dismissed.add(`${instance.id}:${target}`); renderUpgrade(instance); } }, icon('close')));
}

function renderSetup(account, instance) {
    const box = $('#setup');
    box.hidden = !!(account && instance);
    if (box.hidden) return;
    const step = (n, title, text, ...actions) =>
        clear(box).append(
            h('div', { class: 'setup-step' }, `第 ${n} 步（共 2 步）`),
            h('div', { class: 'setup-title' }, title),
            h('div', { class: 'setup-text' }, text),
            h('div', { class: 'setup-actions' }, ...actions)
        );
    if (!account) {
        step(1, '登入帳號', '用微軟帳號登入，就能直接連上輝煌的各個分流。',
            h('button', { type: 'button', class: 'mc primary', onclick: loginMicrosoft }, '微軟正版登入'),
            h('button', { type: 'button', class: 'mc', onclick: addOffline }, '離線帳號'));
    } else {
        step(2, '建立遊戲實例', '一鍵安裝輝煌推薦的 Minecraft 版本、Fabric 與 Brilliant-Server-Mod。',
            h('button', {
                type: 'button',
                class: 'mc primary',
                disabled: creating,
                onclick: async () => {
                    creating = true;
                    renderPlayBar();
                    await quickCreate().catch(fail);
                    creating = false;
                    renderPlayBar();
                }
            }, creating ? '建立中⋯' : '一鍵建立輝煌實例'));
    }
}

function setProgress({ label, doneFiles, totalFiles, doneBytes, totalBytes }) {
    $('#progress').hidden = false;
    const ratio = totalBytes > 0 ? doneBytes / totalBytes : totalFiles > 0 ? doneFiles / totalFiles : 0;
    $('#progress-fill').style.width = `${Math.min(100, ratio * 100).toFixed(1)}%`;
    $('#progress-label').textContent = label || '準備中⋯⋯';
    $('#progress-count').textContent = totalFiles
        ? `${doneFiles} / ${totalFiles}　${fmtBytes(doneBytes)} / ${fmtBytes(totalBytes)}`
        : '';
}

const STAGE_TEXT = {
    version: '解析版本',
    libraries: '下載函式庫',
    assets: '下載資源檔',
    java: '下載 Java 執行環境',
    mods: '同步模組',
    respack: '下載伺服器資源包',
    backup: '備份存檔',
    hook: '執行指令',
    launch: '啟動遊戲'
};

async function play() {
    const instance = selectedInstance();
    if (!instance) return;

    if (isRunning(instance.id)) {
        await api.stop_game({ id: instance.id }).catch(fail);
        return;
    }

    busy = true;
    clearRecentLog();
    renderPlayBar();
    setProgress({ label: '準備中⋯⋯', doneFiles: 0, totalFiles: 0, doneBytes: 0, totalBytes: 0 });
    try {
        const notes = await api.launch_game({ id: instance.id });
        for (const note of notes) toast(note, 'warn');
        const target = shardOf(store.settings?.quickJoin);
        toast(target ? `遊戲已啟動，將直接連線 ${target.label}` : '遊戲已啟動');
        await loadInstances();
        const after = store.settings?.afterLaunch;
        if (after === 'hide') await api.hide_window();
        else if (after === 'exit') await api.exit_launcher();
    } catch (error) {
        fail(error);
    } finally {
        busy = false;
        $('#progress').hidden = true;
        renderPlayBar();
    }
}

function showCrash(code) {
    const lines = recentLog();
    const { findings, exception } = diagnose(lines);
    const content = h('div', { class: 'crash' },
        findings.length
            ? h('div', {}, ...findings.map((f) => h('div', { class: 'crash-item' }, h('b', {}, f.title), h('p', {}, f.detail))))
            : h('p', {}, '沒有找到常見的原因。請按「複製日誌」，貼到輝煌 Discord 的問題回報頻道，管理員會協助查看。'),
        exception ? h('pre', { class: 'crash-exception' }, exception) : '');
    dialog({
        title: `遊戲異常結束（代碼 ${code}）`,
        content,
        buttons: [
            { label: '複製日誌', value: 'copy' },
            { label: '查看日誌', value: 'log' },
            { label: '關閉', value: null }
        ]
    }).then(async (value) => {
        if (value === 'copy') {
            await navigator.clipboard.writeText(lines.join('\n')).then(() => toast('已複製日誌'), fail);
        } else if (value === 'log') {
            location.hash = '#log';
        }
    });
}

function shardTile(shard, status) {
    const selected = store.settings?.quickJoin === shard.host;
    const state = status === undefined ? 'pending' : status ? 'up' : 'down';
    const busiest = Math.max(1, ...statuses.map((item) => item?.online ?? 0));
    const ratio = status?.online != null ? Math.min(1, status.online / busiest) : 0;
    return h(
        'button',
        {
            type: 'button',
            class: `shard glass ${state}`,
            'aria-pressed': selected ? 'true' : 'false',
            title: status ? `${shard.host}\n${status.motd}` : shard.host,
            onclick: async () => {
                await saveSettings({ quickJoin: selected ? '' : shard.host }).catch(fail);
                renderShards();
                renderPlayBar();
            }
        },
        h('span', { class: 'check' }, icon('check'), '直連'),
        h('div', { class: 'name' }, shard.label),
        h(
            'div',
            { class: 'meta' },
            h('span', { class: 'dot' }),
            state === 'pending'
                ? '查詢中⋯'
                : status
                    ? status.online != null
                        ? h('span', {}, h('b', {}, String(status.online)), ` 人　${status.latencyMs}ms`)
                        : h('span', {}, `可連線　${status.latencyMs}ms`)
                    : '目前連不上'
        ),
        h('div', { class: 'bar' }, h('i', { style: `width:${(ratio * 100).toFixed(1)}%` }))
    );
}

function renderShards() {
    clear($('#shards')).append(...SHARDS.map((shard, i) => shardTile(shard, statuses[i])));
    const target = shardOf(store.settings?.quickJoin);
    clear($('#shard-hint')).append(...(target
        ? ['開始遊戲後直接進入 ', h('b', {}, target.label), h('button', {
            type: 'button',
            onclick: async () => {
                await saveSettings({ quickJoin: '' }).catch(fail);
                renderShards();
                renderPlayBar();
            }
        }, '改為進入主畫面')]
        : ['點選分流，開始遊戲後就會直接進入；不選則停在主畫面。']));
    const counted = statuses.filter((status) => status?.online != null);
    const total = counted.reduce((sum, status) => sum + status.online, 0);
    clear($('#shard-total')).append(counted.length ? h('span', {}, '四個分流共 ', h('b', {}, String(total)), ' 人在線') : '');
}

async function refreshShards() {
    const [online, pings] = await Promise.all([
        api.site_json({ path: '/api/online' }).catch(() => null),
        Promise.all(SHARDS.map((shard) => api.ping_server({ host: shard.host }).catch(() => null)))
    ]);
    statuses = pings.map((ping, i) => {
        if (!ping) return null;
        const count = online?.servers?.find((server) => server.name === SHARDS[i].label)?.count;
        return { ...ping, online: Number.isFinite(count) ? count : null };
    });
    renderShards();
}

function toggleInstanceMenu(force) {
    const menu = $('#instance-menu');
    const open = force ?? menu.hidden;
    menu.hidden = !open;
    if (!open) return;
    const current = selectedInstance();
    clear(menu).append(
        ...store.instances.map((instance) =>
            h(
                'button',
                {
                    type: 'button',
                    'aria-current': current?.id === instance.id ? 'true' : 'false',
                    onclick: async () => {
                        toggleInstanceMenu(false);
                        await api.select_instance({ id: instance.id }).catch(fail);
                        store.settings.selectedInstance = instance.id;
                        emit('instances');
                        emit('settings');
                    }
                },
                instanceVisual(instance),
                instance.name,
                h('span', { class: 'sub' }, instance.mcVersion)
            )
        ),
        store.instances.length ? h('hr') : '',
        h('button', { type: 'button', onclick: () => { toggleInstanceMenu(false); location.hash = '#instances'; } }, '管理實例⋯')
    );
}

const FOLD_KEY = 'brilliant.home.fold';

function readFold() {
    try {
        return JSON.parse(localStorage.getItem(FOLD_KEY)) || {};
    } catch {
        return {};
    }
}

function setFold(patch) {
    const fold = { ...readFold(), ...patch };
    try {
        localStorage.setItem(FOLD_KEY, JSON.stringify(fold));
    } catch {}
    applyFold(fold);
}

function applyFold(fold = readFold()) {
    const body = document.body;
    body.classList.toggle('fold-news', !!fold.news);
    body.classList.toggle('fold-dock', !!fold.dock);
    body.classList.toggle('scenic', !!fold.scenic);
    $('#fold-news').setAttribute('aria-expanded', String(!fold.news));
    $('#fold-dock').setAttribute('aria-expanded', String(!fold.dock));
    $('#scenic-toggle').setAttribute('aria-pressed', String(!!fold.scenic));
    $('#scenic-toggle').replaceChildren(icon(fold.scenic ? 'eye-closed' : 'eye'));
}

function initFold() {
    const newsFold = h('button', { type: 'button', class: 'fold', id: 'fold-news', title: '收合公告', onclick: () => setFold({ news: !readFold().news }) }, icon('chevron-down'));
    $('.news').prepend(newsFold);
    $('#fold-dock').addEventListener('click', () => setFold({ dock: !readFold().dock }));
    $('#scenic-toggle').addEventListener('click', () => setFold({ scenic: !readFold().scenic }));
    document.addEventListener('keydown', (e) => {
        if (e.key === 'Escape' && document.body.classList.contains('scenic') && !document.querySelector('dialog[open]')) setFold({ scenic: false });
    });
    applyFold();
}

export function initHome() {
    initFold();
    $('#btn-play').addEventListener('click', play);
    on('launch', () => { if (!busy) play(); });
    $('#instance-pick').classList.add('glass');
    $('#instance-pick').addEventListener('click', (e) => {
        e.stopPropagation();
        toggleInstanceMenu();
    });
    $('#instance-menu').classList.add('glass');
    document.addEventListener('click', (e) => {
        if (!e.target.closest('#instance-menu')) toggleInstanceMenu(false);
    });
    on('instances', renderPlayBar);
    on('accounts', renderPlayBar);
    on('ready', renderPlayBar);
    versionData().then(renderPlayBar, () => {});
    on('settings', () => {
        renderPlayBar();
        renderShards();
    });

    listen('launcher://progress', (p) => {
        if (!busy) return;
        setProgress({ ...p, label: `${STAGE_TEXT[p.stage] ?? p.stage}：${p.label}` });
    });
    listen('launcher://tray-play', () => emit('launch'));
    listen('launcher://exit', async ({ code, seconds }) => {
        await api.show_window().catch(() => {});
        await loadInstances().catch(() => {});
        if (code === 0) toast(`遊戲已結束（遊玩 ${Math.round(seconds / 60)} 分鐘）`);
        else showCrash(code);
    });

    renderPlayBar();
    renderShards();
    refreshShards().catch(() => {});
    setInterval(() => refreshShards().catch(() => {}), PING_INTERVAL_MS);
    initNews();
}
