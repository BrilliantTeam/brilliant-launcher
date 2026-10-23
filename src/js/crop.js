import { api, fileSrc } from './api.js';
import { h } from './ui.js';
import { cleanSvg } from './icons.js';

const loadImage = (src) => new Promise((resolve, reject) => {
    const img = new Image();
    img.crossOrigin = 'anonymous';
    img.onload = () => resolve(img);
    img.onerror = () => reject(new Error('無法讀取這張圖片'));
    img.src = src;
});

const KEEP = /\.(gif|svg)$/i;

async function svgImage(path, longest) {
    const svg = cleanSvg(await api.read_svg({ path }));
    if (!svg) throw new Error('這個 SVG 無法解析');
    const vb = (svg.getAttribute('viewBox') || '0 0 1 1').split(/[\s,]+/).map(Number);
    const scale = longest / Math.max(vb[2] || 1, vb[3] || 1);
    const natW = Math.round((vb[2] || 1) * scale);
    const natH = Math.round((vb[3] || 1) * scale);
    const preview = svg.cloneNode(true);
    preview.setAttribute('width', String(natW));
    preview.setAttribute('height', String(natH));
    preview.setAttribute('color', getComputedStyle(document.documentElement).getPropertyValue('--accent').trim() || '#f6e0a4');
    const img = await loadImage(`data:image/svg+xml;charset=utf-8,${encodeURIComponent(new XMLSerializer().serializeToString(preview))}`);
    return { img, svg, vb, natW };
}

function croppedSvg({ svg, vb, natW }, rect) {
    const k = (vb[2] || 1) / natW;
    const box = [vb[0] + rect.x * k, vb[1] + rect.y * k, rect.w * k, rect.h * k].map((n) => Number(n.toFixed(3))).join(' ');
    const inner = svg.cloneNode(true);
    inner.setAttribute('x', String(vb[0]));
    inner.setAttribute('y', String(vb[1]));
    inner.setAttribute('width', String(vb[2]));
    inner.setAttribute('height', String(vb[3]));
    return `<svg xmlns="http://www.w3.org/2000/svg" viewBox="${box}" preserveAspectRatio="xMidYMid meet">${new XMLSerializer().serializeToString(inner)}</svg>`;
}

export async function importPicture(path, { width, height, title, hint, vectorSvg = false }) {
    const imported = await api.import_image({ path });
    if (vectorSvg && /\.svg$/i.test(imported)) {
        const source = await svgImage(imported, Math.max(width, height));
        const rect = await cropDialog(source.img, { width, height, title, hint, vector: true });
        return rect ? api.save_svg({ text: croppedSvg(source, rect) }) : null;
    }
    if (KEEP.test(imported)) return imported;
    const img = await loadImage(fileSrc(imported));
    const ratio = img.naturalWidth / img.naturalHeight;
    const target = width / height;
    if (Math.abs(ratio - target) / target < 0.03 && img.naturalWidth <= width * 1.5) return imported;
    const data = await cropDialog(img, { width, height, title, hint });
    return data ? api.save_image({ data }) : null;
}

function cropDialog(img, { width, height, title, hint, vector = false }) {
    const base = width === height ? 360 : 520;
    const viewW = Math.round(width >= height ? base : base * width / height);
    const viewH = Math.round(viewW * height / width);
    const canvas = h('canvas', { class: 'crop-canvas', width: viewW * 2, height: viewH * 2, style: `width:${viewW}px;height:${viewH}px` });
    const ctx = canvas.getContext('2d');
    const fillScale = Math.max(viewW / img.naturalWidth, viewH / img.naturalHeight);
    const minScale = Math.min(viewW / img.naturalWidth, viewH / img.naturalHeight);
    const maxScale = fillScale * 4;
    let scale = vector ? minScale : fillScale;
    let x = (viewW - img.naturalWidth * scale) / 2;
    let y = (viewH - img.naturalHeight * scale) / 2;
    const toSlider = (value) => String(Math.round(1000 * Math.log(value / minScale) / Math.log(maxScale / minScale || 2)));
    const zoom = h('input', { type: 'range', min: '0', max: '1000', value: toSlider(scale), 'aria-label': '縮放' });

    const clampAxis = (pos, size, view) => (size <= view ? (view - size) / 2 : Math.min(0, Math.max(view - size, pos)));
    const clamp = () => {
        x = clampAxis(x, img.naturalWidth * scale, viewW);
        y = clampAxis(y, img.naturalHeight * scale, viewH);
    };
    const draw = () => {
        clamp();
        ctx.setTransform(2, 0, 0, 2, 0, 0);
        ctx.clearRect(0, 0, viewW, viewH);
        ctx.imageSmoothingQuality = 'high';
        ctx.drawImage(img, x, y, img.naturalWidth * scale, img.naturalHeight * scale);
    };
    const zoomTo = (next, cx = viewW / 2, cy = viewH / 2) => {
        next = Math.min(maxScale, Math.max(minScale, next));
        x = cx - (cx - x) * next / scale;
        y = cy - (cy - y) * next / scale;
        scale = next;
        zoom.value = toSlider(scale);
        draw();
    };
    zoom.addEventListener('input', () => zoomTo(minScale * Math.pow(maxScale / minScale || 2, Number(zoom.value) / 1000)));
    canvas.addEventListener('wheel', (e) => {
        e.preventDefault();
        const rect = canvas.getBoundingClientRect();
        zoomTo(scale * (e.deltaY < 0 ? 1.1 : 1 / 1.1), e.clientX - rect.left, e.clientY - rect.top);
    }, { passive: false });
    canvas.addEventListener('pointerdown', (e) => {
        canvas.setPointerCapture(e.pointerId);
        const start = { px: e.clientX, py: e.clientY, x, y };
        const move = (ev) => {
            x = start.x + ev.clientX - start.px;
            y = start.y + ev.clientY - start.py;
            draw();
        };
        canvas.addEventListener('pointermove', move);
        canvas.addEventListener('pointerup', () => canvas.removeEventListener('pointermove', move), { once: true });
    });
    draw();

    return new Promise((resolve) => {
        const close = (value) => {
            modal.close();
            modal.remove();
            resolve(value);
        };
        const modal = h('dialog', { class: 'crop-dlg', oncancel: (e) => { e.preventDefault(); close(null); } },
            h('div', { class: 'dlg-body' },
                h('h2', {}, title),
                h('div', { class: 'crop' },
                    h('p', { class: 'hint' }, `${hint}原圖 ${img.naturalWidth}×${img.naturalHeight}，拖曳調整位置，滾輪或下方滑桿縮放；縮小到底可以看到整張圖，空白處會是透明的。`),
                    canvas,
                    h('label', { class: 'crop-zoom' }, '縮放', zoom))),
            h('div', { class: 'dlg-foot' },
                h('button', { type: 'button', class: 'mc', onclick: () => close(null) }, '取消'),
                h('button', { type: 'button', class: 'mc primary', onclick: () => close('ok') }, '確定')));
        document.body.append(modal);
        modal.showModal();
    }).then((answer) => {
        if (answer !== 'ok') return null;
        if (vector) return { x: -x / scale, y: -y / scale, w: viewW / scale, h: viewH / scale };
        const out = h('canvas', { width, height });
        const octx = out.getContext('2d');
        octx.imageSmoothingQuality = 'high';
        const k = width / viewW;
        octx.drawImage(img, x * k, y * k, img.naturalWidth * scale * k, img.naturalHeight * scale * k);
        return width === height || scale < fillScale * 0.999 ? out.toDataURL('image/png') : out.toDataURL('image/jpeg', 0.9);
    });
}
