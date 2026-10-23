const DAY = 86400;
const WEEKDAYS = ['星期日', '星期一', '星期二', '星期三', '星期四', '星期五', '星期六'];
const SHARDS = { 's1.brilliantw.net': '炒飯服', 's2.brilliantw.net': '書碼服', 's3.brilliantw.net': '羽森服', 's4.brilliantw.net': '恐龍服' };
const DIMENSIONS = { 'minecraft:overworld': '主世界', 'minecraft:the_nether': '地獄', 'minecraft:the_end': '終界' };

const biomeIs = (re) => (m) => re.test(m.biome ?? '');
const night = (m) => m.dayTime >= 13000 && m.dayTime < 23000;
const THEMES = [
    { key: 'night', titles: ['夜空下的世界', '星空與月光', '當夜幕降臨'], test: (m) => m.dimension === 'minecraft:overworld' && night(m) && m.weather === 'clear' },
    { key: 'dusk', titles: ['黃昏時分', '夕陽與晨光', '天色漸變的時刻'], test: (m) => m.dimension === 'minecraft:overworld' && m.weather === 'clear' && ((m.dayTime >= 11500 && m.dayTime < 13000) || m.dayTime >= 23000) },
    { key: 'rain', titles: ['雨天的記憶', '下雨的日子', '雷雨交加'], test: (m) => m.weather !== 'clear' && m.dimension === 'minecraft:overworld' },
    { key: 'nether', titles: ['地獄之旅', '在烈焰之間', '深入地獄'], test: (m) => m.dimension === 'minecraft:the_nether' },
    { key: 'end', titles: ['終界遠征', '虛空的盡頭'], test: (m) => m.dimension === 'minecraft:the_end' },
    { key: 'cherry', titles: ['櫻花林的時光', '粉色的季節', '櫻花盛開'], test: biomeIs(/cherry/) },
    { key: 'snow', titles: ['冰雪世界', '雪地裡的足跡', '白色的風景'], test: biomeIs(/snow|frozen|ice|:grove$|jagged/) },
    { key: 'ocean', titles: ['海與岸邊', '海風吹來的地方', '藍色的記憶'], test: biomeIs(/ocean|beach|river/) },
    { key: 'desert', titles: ['沙漠與惡地', '黃沙之上'], test: biomeIs(/desert|badlands/) },
    { key: 'jungle', titles: ['叢林探險', '綠意深處'], test: biomeIs(/jungle|bamboo/) },
    { key: 'swamp', titles: ['沼澤與紅樹林', '濕地漫遊'], test: biomeIs(/swamp|mangrove/) },
    { key: 'mushroom', titles: ['蘑菇島奇遇'], test: biomeIs(/mushroom/) },
    { key: 'forest', titles: ['森林漫步', '林間小徑', '樹下的時光'], test: biomeIs(/forest|taiga|birch/) },
    { key: 'deep', titles: ['地底探險', '深入地心', '洞穴裡的光'], test: (m) => /deep_dark|lush_caves|dripstone/.test(m.biome ?? '') || (m.dimension === 'minecraft:overworld' && m.y < 40) },
    { key: 'high', titles: ['從高處看世界', '雲端之上'], test: (m) => m.y >= 180 },
    { key: 'friends', titles: ['和朋友們一起', '大家都在', '熱鬧的時刻'], test: (m) => m.nearby >= 2 }
];

const local = (t) => new Date(t * 1000);
const dateKey = (t) => { const d = local(t); return `${d.getFullYear()}-${d.getMonth() + 1}-${d.getDate()}`; };
export const fmtDate = (t) => { const d = local(t); return `${d.getFullYear()} 年 ${d.getMonth() + 1} 月 ${d.getDate()} 日`; };
const dayOfYear = (t) => { const d = local(t); return Math.round((Date.UTC(d.getFullYear(), d.getMonth(), d.getDate()) - Date.UTC(d.getFullYear(), 0, 0)) / 864e5); };
const pick = (list, seed) => list[Math.abs(seed) % list.length];
const span = (shots) => {
    const times = shots.map((s) => s.time);
    const [a, b] = [Math.min(...times), Math.max(...times)];
    return dateKey(a) === dateKey(b) ? fmtDate(a) : `${fmtDate(a)} ～ ${fmtDate(b)}`;
};

function card(key, title, subtitle, shots, seed) {
    const sorted = [...shots].sort((a, b) => a.time - b.time);
    return { key, title, subtitle, shots: sorted, cover: sorted[Math.abs(seed) % sorted.length] };
}

export function caption(shot) {
    const m = shot.meta;
    if (!m) return '';
    const place = [SHARDS[m.server], DIMENSIONS[m.dimension] ?? '', m.biomeName].filter(Boolean).join('・');
    const tod = m.dayTime == null ? '' : night(m) ? '夜晚' : m.dayTime >= 11500 && m.dayTime < 13000 ? '黃昏' : m.dayTime >= 23000 ? '黎明' : '白天';
    return [place, `${m.x}, ${m.y}, ${m.z}`, tod, m.weather === 'rain' ? '下雨' : m.weather === 'thunder' ? '雷雨' : ''].filter(Boolean).join('｜');
}

export function memories(shots, now, limit = 8) {
    const seed = Math.floor(now / DAY);
    const out = [];
    const year = local(now).getFullYear();
    const today = dayOfYear(now);

    const onThisDay = shots.filter((s) => local(s.time).getFullYear() < year && Math.abs(dayOfYear(s.time) - today) <= 2);
    if (onThisDay.length) {
        const years = [...new Set(onThisDay.map((s) => year - local(s.time).getFullYear()))].sort((a, b) => a - b);
        const title = years.length > 1 ? '這一天，在不同的年份' : `${years[0]} 年前的今天`;
        out.push(card('onthisday', title, years.length > 1 ? `${years.join('、')} 年前的這幾天` : span(onThisDay), onThisDay, seed));
    }

    const monthAgo = shots.filter((s) => s.time <= now - 27 * DAY && s.time >= now - 33 * DAY);
    if (monthAgo.length >= 2) out.push(card('month-ago', '一個月前的這時候', span(monthAgo), monthAgo, seed));

    const days = {};
    for (const s of shots) if (s.time < now - 7 * DAY) (days[dateKey(s.time)] ??= []).push(s);
    const busy = Object.values(days).filter((d) => d.length >= 3).sort((a, b) => b[0].time - a[0].time);
    if (busy.length) {
        const day = pick(busy, seed);
        const t = day[0].time;
        const title = pick([`還記得那個${WEEKDAYS[local(t).getDay()]}嗎？`, '還記得這一天嗎……', '回顧那一刻'], seed);
        out.push(card(`day-${dateKey(t)}`, title, `${fmtDate(t)}・${day[0].instanceName}`, day, seed));
    }

    const themed = [];
    const withMeta = shots.filter((s) => s.meta);
    for (const theme of THEMES) {
        const hits = withMeta.filter((s) => theme.test(s.meta));
        if (hits.length >= 3) themed.push(card(`theme-${theme.key}`, pick(theme.titles, seed), span(hits), hits, seed));
    }
    for (const [host, label] of Object.entries(SHARDS)) {
        const hits = withMeta.filter((s) => s.meta.server === host);
        if (hits.length >= 6) themed.push(card(`shard-${host}`, `在${label}的日子`, span(hits), hits, seed));
    }
    themed.sort((a, b) => b.shots.length - a.shots.length);
    const rotate = themed.length ? seed % themed.length : 0;
    out.push(...themed.slice(rotate), ...themed.slice(0, rotate));

    const d = local(now);
    const lastMonth = new Date(d.getFullYear(), d.getMonth() - 1, 1);
    const inLastMonth = shots.filter((s) => { const x = local(s.time); return x.getFullYear() === lastMonth.getFullYear() && x.getMonth() === lastMonth.getMonth(); });
    if (inLastMonth.length >= 4) out.push(card('last-month', `${lastMonth.getFullYear()} 年 ${lastMonth.getMonth() + 1} 月精選`, `這個月拍了 ${inLastMonth.length} 張`, inLastMonth, seed));

    const first = shots.reduce((a, s) => (!a || s.time < a.time ? s : a), null);
    if (first && shots.length >= 20 && first.time < now - 30 * DAY) out.push(card('first', '一切從這裡開始', `你的第一張截圖・${fmtDate(first.time)}`, [first], 0));

    return out.slice(0, limit);
}

export function byMonth(shots) {
    const groups = [];
    for (const s of shots) {
        const d = local(s.time);
        const key = `${d.getFullYear()}-${d.getMonth()}`;
        if (groups.at(-1)?.key !== key) groups.push({ key, year: d.getFullYear(), month: d.getMonth() + 1, shots: [] });
        groups.at(-1).shots.push(s);
    }
    return groups;
}
