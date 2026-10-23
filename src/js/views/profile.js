import { api, openUrl } from '../api.js';
import { $, clear, h, guard } from '../ui.js';
import { icon } from '../icons.js';
import { avatarUrl, on, selectedAccount } from '../store.js';

const LINK_GUIDE = `https://www.brilliantw.net/${encodeURIComponent('連結帳號')}/`;
const linkState = new Map();

const loaded = new Map();

function formatValue(value, format) {
    if (format === 'duration') {
        const hours = Math.floor(value / 72000);
        return hours >= 1 ? `${hours.toLocaleString('zh-TW')} 小時` : `${Math.floor(value / 1200)} 分`;
    }
    if (format === 'distance') return `${(value / 100000).toLocaleString('zh-TW', { maximumFractionDigits: 1 })} 公里`;
    return Number(value || 0).toLocaleString('zh-TW');
}

const fmtDate = (ms) => (ms ? new Date(ms).toLocaleDateString('zh-TW') : '—');

function statCard(label, value, sub) {
    return h('div', { class: 'stat' }, h('div', { class: 'stat-value' }, value), h('div', { class: 'stat-label' }, label), sub ? h('div', { class: 'stat-sub' }, sub) : null);
}

function renderProfile(account, data) {
    const box = clear($('#profile-body'));
    const keys = Object.keys(data.metrics);
    const totals = Object.fromEntries(keys.map((key) => [key, data.servers.reduce((sum, s) => sum + (s.stats[key]?.value || 0), 0)]));
    const visible = keys.filter((key) => data.servers.some((s) => s.stats[key]));
    const firsts = data.servers.map((s) => s.firstPlayed).filter(Boolean);
    const lasts = data.servers.map((s) => s.lastPlayed).filter(Boolean);

    box.append(
        h('div', { class: 'profile-head' },
            h('img', { class: 'profile-avatar', src: avatarUrl(account), alt: '', 'data-avatar': account.id }),
            h('div', { class: 'grow' },
                h('div', { class: 'profile-name' }, data.name || account.name,
                    data.rank ? h('span', { class: 'profile-rank', style: `--rank:${data.rank.color || 'var(--accent)'}` }, data.rank.label) : null,
                    data.banned ? h('span', { class: 'badge warn' }, '已停權') : null),
                h('div', { class: 'meta' },
                    `首次遊玩 ${fmtDate(firsts.length ? Math.min(...firsts) : 0)}｜最後上線 ${fmtDate(lasts.length ? Math.max(...lasts) : 0)}｜去過 ${data.servers.length} 個分流`)),
            h('button', { type: 'button', class: 'mc', onclick: () => openUrl(data.url) }, icon('external-link'), '在官網查看完整資料')),
        data.medals
            ? h('div', { class: 'medals' },
                h('span', {}, '金牌 ', h('b', {}, data.medals.gold)),
                h('span', {}, '銀牌 ', h('b', {}, data.medals.silver)),
                h('span', {}, '銅牌 ', h('b', {}, data.medals.bronze)),
                h('span', {}, '綜合歷程 ', h('b', {}, Number(data.medals.score || 0).toLocaleString('zh-TW'))))
            : '',
        h('div', { class: 'stats-grid' }, ...visible.map((key) => statCard(data.metrics[key].label, formatValue(totals[key], data.metrics[key].format), '各分流加總'))),
        h('h2', {}, '各分流'),
        h('div', { class: 'rows' }, ...data.servers.map((server) => h('div', { class: 'row profile-server' },
            h('div', { class: 'grow' },
                h('div', { class: 'name' }, server.name),
                h('div', { class: 'meta' }, `最後上線 ${fmtDate(server.lastPlayed)}`)),
            ...visible.filter((key) => server.stats[key]).slice(0, 4).map((key) => h('div', { class: 'mini-stat' },
                h('b', {}, formatValue(server.stats[key].value, data.metrics[key].format)),
                h('small', {}, `${data.metrics[key].label}${server.stats[key].rank ? `・第 ${server.stats[key].rank} 名` : ''}`)))))),
        visible.length < keys.length
            ? h('p', { class: 'hint' }, '部分資料依你在官網的隱私設定隱藏，可到官網統計頁登入 Discord 後調整。')
            : ''
    );
}

async function showLinkHint(account) {
    if (!linkState.has(account.uuid)) {
        linkState.set(account.uuid, await api.site_json({ path: `/api/launcher/linked?uuid=${account.uuid.replace(/-/g, '')}` }).then((d) => d.linked, () => null));
    }
    if (linkState.get(account.uuid) !== false || selectedAccount()?.id !== account.id || $('#profile-body .link-hint')) return;
    $('#profile-body').prepend(h('div', { class: 'link-hint' },
        icon('discord'),
        h('div', { class: 'grow' },
            h('b', {}, '你的 Minecraft 帳號還沒綁定 Discord'),
            h('div', {}, '綁定後才能投稿「玩家足跡」，也能在官網查詢自己的資料。在遊戲內輸入 /discord link 就能開始。')),
        h('div', { class: 'link-actions' },
            h('button', { type: 'button', class: 'mc primary', onclick: () => openUrl(LINK_GUIDE) }, '查看綁定教學'),
            h('button', { type: 'button', class: 'mc', onclick: () => { linkState.delete(account.uuid); loaded.delete(account.uuid.replace(/-/g, '')); render(); } }, icon('reload'), '已綁定？重新檢查'))));
}

async function render() {
    const account = selectedAccount();
    const box = clear($('#profile-body'));
    $('#btn-profile-refresh').hidden = true;
    if (!account) {
        box.append(h('div', { class: 'empty-state' }, '先登入帳號，就能看到你在輝煌的遊玩紀錄。'));
        return;
    }
    if (account.kind !== 'msa') {
        box.append(h('div', { class: 'empty-state' }, '離線帳號沒有正版 UUID，無法對應伺服器紀錄。請改用微軟正版登入。'));
        return;
    }
    const uuid = account.uuid.replace(/-/g, '');
    let data = loaded.get(uuid);
    if (!data) {
        box.append(h('div', { class: 'stats-grid' }, ...Array.from({ length: 6 }, () => h('div', { class: 'stat skeleton' }))));
        data = await api.site_json({ path: `/mcstats/api/player?uuid=${uuid}` }).catch(() => null);
        if (data) loaded.set(uuid, data);
        if (selectedAccount()?.id !== account.id) return;
    }
    if (!data) {
        clear(box).append(h('div', { class: 'empty-state' }, '暫時連不上官網，請稍後再試。'));
        $('#btn-profile-refresh').hidden = false;
        return;
    }
    if (!data.servers.length) clear(box).append(h('div', { class: 'empty-state' }, '還沒有你在輝煌的遊玩紀錄，進伺服器玩一陣子再回來看看吧！'));
    else {
        renderProfile(account, data);
        $('#btn-profile-refresh').hidden = false;
    }
    showLinkHint(account);
}

export function initProfile() {
    const visible = () => location.hash === '#profile';
    on('accounts', () => { if (visible()) render(); });
    on('settings', () => { if (visible()) render(); });
    addEventListener('hashchange', () => { if (visible()) render(); });
    $('#btn-profile-refresh').addEventListener('click', guard(() => {
        loaded.clear();
        linkState.clear();
        render();
    }));
    if (visible()) render();
}
