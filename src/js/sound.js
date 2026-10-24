const VOICES = {
    block: [{ type: 'square', from: 620, to: 240, len: 0.07 }],
    button: [{ type: 'triangle', from: 1100, to: 900, len: 0.04 }, { type: 'triangle', from: 700, to: 600, len: 0.05, at: 0.045 }],
    xp: [{ type: 'sine', from: 1320, to: 1760, len: 0.09 }, { type: 'sine', from: 1980, to: 2350, len: 0.08, at: 0.05 }],
    wood: [{ type: 'triangle', from: 260, to: 120, len: 0.09 }, { type: 'square', from: 180, to: 90, len: 0.04, gain: 0.4 }]
};

let audio = null;

export function playClick(type = 'block', volume = 50) {
    const level = Math.min(100, Math.max(0, Number(volume) || 0)) / 100;
    if (!level) return;
    try {
        audio ??= new AudioContext();
        const now = audio.currentTime;
        for (const voice of VOICES[type] ?? VOICES.block) {
            const t = now + (voice.at ?? 0);
            const osc = audio.createOscillator();
            const gain = audio.createGain();
            osc.type = voice.type;
            osc.frequency.setValueAtTime(voice.from, t);
            osc.frequency.exponentialRampToValueAtTime(voice.to, t + voice.len * 0.8);
            gain.gain.setValueAtTime(0.1 * level * (voice.gain ?? 1), t);
            gain.gain.exponentialRampToValueAtTime(0.0001, t + voice.len);
            osc.connect(gain).connect(audio.destination);
            osc.start(t);
            osc.stop(t + voice.len + 0.01);
        }
    } catch {}
}
