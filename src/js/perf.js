import { api } from './api.js';
import { dialog, fail, h, toast } from './ui.js';
import { loadInstances } from './store.js';
import { isIntegratedGpu } from './services.js';
import { PRESETS } from './views/mods.js';

const LABELS = {
    renderDistance: '顯示距離',
    graphicsMode: '畫質',
    particles: '粒子效果',
    biomeBlendRadius: '生態域混合',
    entityShadows: '實體陰影',
    cloudStatus: '雲朵'
};
const VALUE_TEXT = {
    graphicsMode: { 0: '流暢', 1: '精緻', 2: '極致' },
    particles: { 0: '全部', 1: '減少', 2: '最少' },
    entityShadows: { true: '開', false: '關' },
    cloudStatus: { '"true"': '精緻', '"fast"': '流暢', '"false"': '關' }
};
const show = (key, value) => VALUE_TEXT[key]?.[value] ?? (key === 'renderDistance' ? `${value} 區塊` : value);

export function recommend({ options = {}, mods = [], sessions = [], memoryMb = 0, system = {}, integrated = false, fabric = true }) {
    const last = sessions.at(-1) ?? null;
    const strain = last ? last.avgFps < 45 || last.low1 < 25 : integrated;
    const changes = [];
    const set = (key, to, why) => {
        if (options[key] !== undefined && options[key] !== String(to)) changes.push({ key, from: options[key], to: String(to), why });
    };
    const rd = Number(options.renderDistance);
    if (strain) {
        const cap = integrated ? 8 : 12;
        if (rd > cap) set('renderDistance', cap, '顯示距離是最吃效能的設定，伺服器上 8～12 區塊就很夠用');
        if (options.graphicsMode === '2') set('graphicsMode', 1, '「極致」畫質非常耗顯示卡，「精緻」看起來差不多');
        else if (integrated && options.graphicsMode === '1') set('graphicsMode', 0, '內建顯示卡用「流暢」會順很多');
        if (options.particles === '0') set('particles', 1, '大量粒子（爆炸、農場）容易瞬間掉幀');
        if (Number(options.biomeBlendRadius) > 2) set('biomeBlendRadius', 1, '混合範圍越大，載入新區塊越卡');
        if (integrated && options.entityShadows === 'true') set('entityShadows', false, '生物多的地方能省下不少效能');
        if (options.cloudStatus === '"true"') set('cloudStatus', '"fast"', '立體雲在內建顯示卡上很吃效能');
    } else if (last && last.low1 >= 90 && rd && rd < 12 && !integrated) {
        set('renderDistance', 12, `你的電腦還有餘裕（最低也有 ${last.low1} FPS），可以看得更遠`);
    }
    let memory = null;
    const rec = system.recommendedMb || 0;
    const perMinute = last && last.seconds > 0 ? last.stutters / (last.seconds / 60) : 0;
    if (rec && system.memoryMb && memoryMb > system.memoryMb * 0.6) memory = { from: memoryMb, to: rec, why: '分給遊戲的記憶體太多，系統其他程式沒空間，反而會卡' };
    else if (rec && memoryMb < rec && perMinute > 2) memory = { from: memoryMb, to: rec, why: `上次遊玩平均每分鐘卡頓 ${perMinute.toFixed(1)} 次，多半是記憶體不夠在頻繁回收` };
    const sodium = fabric && !mods.some((id) => ['sodium', 'embeddium', 'optifabric'].includes(id));
    return { last, changes, memory, sodium };
}

export async function perfCheck(instance) {
    const report = await api.perf_report({ id: instance.id }).catch((error) => { fail(error); return null; });
    if (!report) return;
    const fabric = instance.loader === 'fabric';
    const { last, changes, memory, sodium } = recommend({ ...report, integrated: isIntegratedGpu(), fabric });
    const rows = changes.map((c) => ({ c, input: h('input', { type: 'checkbox', checked: true }) }));
    const memInput = memory ? h('input', { type: 'checkbox', checked: true }) : null;
    const stat = (label, value) => h('div', { class: 'perf-stat' }, h('b', {}, value), h('span', {}, label));
    const content = h('div', { class: 'perf' },
        last
            ? h('div', { class: 'perf-stats' },
                stat('平均 FPS', last.avgFps), stat('最低 1% FPS', last.low1), stat('卡頓次數', last.stutters), stat('遊玩分鐘', Math.round(last.seconds / 60)))
            : h('p', { class: 'hint' }, fabric && !instance.general
                ? '還沒有遊玩紀錄。Brilliant-Server-Mod 會在你玩的時候記錄 FPS，下次就能給出更準的建議；現在先依你的硬體判斷。'
                : '這個實例沒有 Brilliant-Server-Mod，無法記錄 FPS，以下依你的硬體判斷。'),
        h('p', { class: 'hint' }, `電腦：${Math.round((report.system.memoryMb || 0) / 1024)} GB 記憶體、${report.system.cpus} 核心${isIntegratedGpu() ? '、內建顯示卡' : ''}；目前分給遊戲 ${report.memoryMb} MB。`),
        rows.length || memory
            ? h('div', { class: 'crash-fixes' },
                h('b', {}, '建議調整：'),
                ...rows.map(({ c, input }) => h('label', {}, input, h('span', {}, `${LABELS[c.key]}：${show(c.key, c.from)} → ${show(c.key, c.to)}`, h('small', {}, c.why)))),
                memory ? h('label', {}, memInput, h('span', {}, `記憶體：${memory.from} → ${memory.to} MB`, h('small', {}, memory.why))) : '')
            : h('p', {}, last ? '目前的設定很適合你的電腦，不需要調整。' : '目前的設定看起來沒問題。'),
        sodium ? h('p', { class: 'hint' }, '還沒有安裝 Sodium。「效能優化包」通常能讓 FPS 提升 2～3 倍，強烈建議安裝。') : '');
    const value = await dialog({
        title: `效能健檢｜${instance.name}`,
        content,
        buttons: [
            ...(rows.length || memory ? [{ label: '套用勾選的調整', value: 'apply' }] : []),
            ...(sodium ? [{ label: '安裝效能優化包', value: 'sodium' }] : []),
            { label: '關閉', value: null }
        ]
    });
    if (value === 'apply') {
        const options = Object.fromEntries(rows.filter((r) => r.input.checked).map(({ c }) => [c.key, c.to]));
        await api.apply_perf({ id: instance.id, options, memoryMb: memInput?.checked ? memory.to : null }).then(() => toast('已套用，下次開始遊戲生效'), fail);
        await loadInstances().catch(() => {});
    } else if (value === 'sodium') {
        toast('正在安裝效能優化包⋯');
        const failed = [];
        for (const [kind, project, title] of PRESETS[0].items) {
            await api.content_install({ id: instance.id, kind, project }).catch(() => failed.push(title));
        }
        toast(failed.length ? `效能優化包已安裝，${failed.join('、')} 暫時沒有支援這個版本` : '效能優化包安裝完成', failed.length ? 'warn' : '');
    }
}

export async function perfAfterExit(id, seconds) {
    const report = await api.perf_report({ id }).catch(() => null);
    const last = report?.sessions?.at(-1);
    if (!last || last.start < Date.now() / 1000 - seconds - 120 || last.avgFps >= 40) return;
    toast(`這次平均只有 ${last.avgFps} FPS。到實例的「更多 → 效能健檢」，啟動器會幫你找出最適合的設定。`, 'warn');
}
