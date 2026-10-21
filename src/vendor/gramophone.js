(function () {
    var script = document.currentScript;
    var SOUND_BASE = (script && script.dataset.base) || '/gramophone/sound/';
    var RENDER_RATE = 48000;
    var decoder = null;
    var buffers = new Map();

    function decodeAt() {
        var Offline = window.OfflineAudioContext || window.webkitOfflineAudioContext;
        if (!decoder) decoder = new Offline(1, 1, RENDER_RATE);
        return decoder;
    }

    function buffer(file) {
        if (!buffers.has(file)) {
            buffers.set(file, (window.walkerFetch || fetch)(SOUND_BASE + file + '.ogg', { credentials: 'same-origin' })
                .then(function (res) { return res.ok ? res.arrayBuffer() : null; })
                .then(function (raw) { return raw && decodeAt().decodeAudioData(raw); })
                .catch(function () { return null; }));
        }
        return buffers.get(file);
    }

    function mix(tl, loaded) {
        var length = Math.max(1, Math.ceil(tl.durationMs / 1000 * RENDER_RATE));
        var buffer = decodeAt().createBuffer(2, length, RENDER_RATE);
        var left = buffer.getChannelData(0);
        var right = buffer.getChannelData(1);

        tl.frames.forEach(function (frame) {
            var sound = loaded[frame[1]];
            if (!sound) return;

            var volume = frame[2];
            var distance = Math.hypot(frame[4], frame[5], frame[6]);
            var fade = 1 - distance / (16 * Math.max(1, volume));
            if (fade <= 0) return;

            var gain = Math.min(1, volume) * fade;
            var pan = distance > 0 ? Math.max(-1, Math.min(1, frame[4] / distance)) : 0;
            var angle = (pan + 1) * Math.PI / 4;
            var gainL = gain * Math.cos(angle);
            var gainR = gain * Math.sin(angle);

            var srcL = sound.getChannelData(0);
            var srcR = sound.numberOfChannels > 1 ? sound.getChannelData(1) : srcL;
            var step = Math.max(0.05, frame[3] || 1) * sound.sampleRate / RENDER_RATE;
            var at = Math.round(frame[0] / 1000 * RENDER_RATE);
            var count = Math.min(Math.floor(sound.length / step), length - at);

            for (var i = 0; i < count; i++) {
                var pos = i * step;
                var index = pos | 0;
                var frac = pos - index;
                var next = index + 1 < sound.length ? index + 1 : index;
                left[at + i] += (srcL[index] + (srcL[next] - srcL[index]) * frac) * gainL;
                right[at + i] += (srcR[index] + (srcR[next] - srcR[index]) * frac) * gainR;
            }
        });

        var peak = 0;
        for (var i = 0; i < length; i++) peak = Math.max(peak, Math.abs(left[i]), Math.abs(right[i]));
        if (peak > 1) {
            for (var j = 0; j < length; j++) { left[j] /= peak; right[j] /= peak; }
        }
        return buffer;
    }

    window.GramophoneEngine = { RENDER_RATE: RENDER_RATE, decodeAt: decodeAt, buffer: buffer, mix: mix };
})();
