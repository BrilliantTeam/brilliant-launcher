import { idle } from './store.js';

const KINDS = {
    snow: { density: 1 / 9000, size: [2, 4], vy: [0.4, 1.1], vx: [-0.3, 0.3], sway: 0.6, colors: ['#ffffff', '#e7f1ff'], alpha: [0.6, 0.95] },
    rain: { density: 1 / 5000, size: [2, 2], length: [10, 18], vy: [9, 13], vx: [-1.6, -1.1], colors: ['#a9c7ff'], alpha: [0.25, 0.45] },
    sakura: { density: 1 / 16000, size: [3, 5], vy: [0.5, 1.2], vx: [0.3, 1], sway: 1.2, colors: ['#ffc4d8', '#ff9ec0', '#ffe0ea'], alpha: [0.7, 0.95] },
    leaves: { density: 1 / 18000, size: [3, 5], vy: [0.6, 1.3], vx: [-0.6, 0.4], sway: 1.4, colors: ['#e8590c', '#f08c00', '#c92a2a', '#fab005'], alpha: [0.75, 0.95] },
    fireflies: { density: 1 / 26000, size: [2, 3], vy: [-0.25, 0.25], vx: [-0.25, 0.25], glow: true, colors: ['#d8f5a2', '#ffec99'], alpha: [0.2, 1] }
};

const SEASONS = ['snow', 'snow', 'sakura', 'sakura', 'sakura', 'rain', 'fireflies', 'fireflies', 'leaves', 'leaves', 'leaves', 'snow'];

const between = ([a, b]) => a + Math.random() * (b - a);

let current = 'none';
let particles = [];
let frame = 0;
let canvas = null;
let ctx = null;

function spawn(kind, top) {
    const conf = KINDS[kind];
    return {
        x: Math.random() * canvas.width,
        y: top ? -20 : Math.random() * canvas.height,
        size: Math.round(between(conf.size)),
        length: conf.length ? between(conf.length) : 0,
        vx: between(conf.vx),
        vy: between(conf.vy),
        phase: Math.random() * Math.PI * 2,
        color: conf.colors[Math.floor(Math.random() * conf.colors.length)],
        alpha: between(conf.alpha)
    };
}

function resize() {
    if (!canvas) return;
    canvas.width = innerWidth;
    canvas.height = innerHeight;
    const conf = KINDS[current];
    if (!conf) return;
    const want = Math.min(260, Math.round(canvas.width * canvas.height * conf.density));
    particles = Array.from({ length: want }, () => spawn(current, false));
}

function step(time) {
    frame = requestAnimationFrame(step);
    if (idle()) return;
    const conf = KINDS[current];
    ctx.clearRect(0, 0, canvas.width, canvas.height);
    for (const p of particles) {
        p.phase += 0.02;
        p.x += p.vx + (conf.sway ? Math.sin(p.phase) * conf.sway * 0.3 : 0);
        p.y += p.vy;
        if (conf.glow) {
            p.vx = Math.max(-0.4, Math.min(0.4, p.vx + (Math.random() - 0.5) * 0.04));
            p.vy = Math.max(-0.4, Math.min(0.4, p.vy + (Math.random() - 0.5) * 0.04));
        }
        const out = p.y > canvas.height + 20 || p.x < -30 || p.x > canvas.width + 30 || p.y < -40;
        if (out) Object.assign(p, spawn(current, !conf.glow));
        ctx.globalAlpha = conf.glow ? p.alpha * (0.55 + 0.45 * Math.sin(time / 500 + p.phase * 20)) : p.alpha;
        ctx.fillStyle = p.color;
        const x = Math.round(p.x);
        const y = Math.round(p.y);
        if (p.length) ctx.fillRect(x, y, p.size, Math.round(p.length));
        else ctx.fillRect(x, y, p.size, p.size);
        if (conf.glow) {
            ctx.globalAlpha *= 0.25;
            ctx.fillRect(x - p.size, y - p.size, p.size * 3, p.size * 3);
        }
    }
    ctx.globalAlpha = 1;
}

export function setWeather(want) {
    const kind = want === 'seasonal' ? SEASONS[new Date().getMonth()] : want;
    const next = KINDS[kind] ? kind : 'none';
    if (next === current) return;
    current = next;
    canvas ??= document.getElementById('weather');
    ctx ??= canvas.getContext('2d');
    cancelAnimationFrame(frame);
    canvas.hidden = next === 'none';
    if (next === 'none') {
        particles = [];
        ctx.clearRect(0, 0, canvas.width, canvas.height);
        return;
    }
    resize();
    frame = requestAnimationFrame(step);
}

addEventListener('resize', resize);
