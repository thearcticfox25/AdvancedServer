'use strict';

class VideoPlayerWindow extends AppWindow {
    constructor(opts = {}) {
        super({
            width: 780, height: 520,
            window_name: opts.window_name || 'Video Player',
            app_class:   'videoplayer',
            icon_name:   '📹',
            ...opts,
        });
        this._src        = opts.src || '';
        this._flashTimer = null;
    }

    getCurrentAddress() { return this._src || '(blank)'; }

    renderContent() {
        const safeAddr = this._esc(this._src);
        this.contentEl.innerHTML = `
<div class="vp-root">
  <div class="vp-urlbar">
    <button class="vp-btn vp-load-btn" id="vp-load-${this.id}" title="Load video">&#8592;</button>
    <input  class="vp-addr" id="vp-addr-${this.id}" type="text"
            value="${safeAddr}" placeholder="URL or path to video…" spellcheck="false">
  </div>
  <div class="vp-screen" id="vp-screen-${this.id}">
    <video class="vp-video" id="vp-vid-${this.id}" preload="metadata"${this._src ? '' : ' style="display:none"'}></video>
    <div class="vp-overlay" id="vp-ov-${this.id}">
      <div class="vp-overlay-icon" id="vp-ovc-${this.id}"></div>
    </div>
    <div class="vp-blank" id="vp-blank-${this.id}"${this._src ? ' style="display:none"' : ''}>
      <div class="vp-blank-icon">&#128249;</div>
      <div class="vp-blank-txt">Enter a video URL above and press &#8592; or Return</div>
    </div>
  </div>
  <div class="vp-seekwrap" id="vp-seekwrap-${this.id}">
    <div class="vp-seek-fill" id="vp-fill-${this.id}"></div>
    <input class="vp-seek" id="vp-seek-${this.id}" type="range" min="0" max="1000" step="1" value="0">
  </div>
  <div class="vp-controls">
    <button class="vp-btn vp-play-btn" id="vp-play-${this.id}" title="Play / Pause">&#9654;</button>
    <button class="vp-btn"             id="vp-rew-${this.id}"  title="Back 10 s">&#8722;10</button>
    <button class="vp-btn"             id="vp-fwd-${this.id}"  title="Forward 10 s">+10</button>
    <span class="vp-time" id="vp-time-${this.id}">0:00 / 0:00</span>
    <span class="vp-spacer"></span>
    <button class="vp-btn vp-mute-btn" id="vp-mute-${this.id}" title="Mute / Unmute">&#128266;</button>
    <input  class="vp-vol" id="vp-vol-${this.id}" type="range" min="0" max="1" step="0.05" value="1" title="Volume">
    <select class="vp-speed" id="vp-spd-${this.id}" title="Playback speed">
      <option value="0.5">0.5&#215;</option>
      <option value="0.75">0.75&#215;</option>
      <option value="1" selected>1&#215;</option>
      <option value="1.25">1.25&#215;</option>
      <option value="1.5">1.5&#215;</option>
      <option value="2">2&#215;</option>
    </select>
  </div>
</div>`;

        this._setupEvents();
        if (this._src) this._loadSrc(this._src);
    }

    _setupEvents() {
        const vid   = document.getElementById(`vp-vid-${this.id}`);
        const addr  = document.getElementById(`vp-addr-${this.id}`);
        const loadB = document.getElementById(`vp-load-${this.id}`);
        const play  = document.getElementById(`vp-play-${this.id}`);
        const rew   = document.getElementById(`vp-rew-${this.id}`);
        const fwd   = document.getElementById(`vp-fwd-${this.id}`);
        const mute  = document.getElementById(`vp-mute-${this.id}`);
        const vol   = document.getElementById(`vp-vol-${this.id}`);
        const seek  = document.getElementById(`vp-seek-${this.id}`);
        const fill  = document.getElementById(`vp-fill-${this.id}`);
        const time  = document.getElementById(`vp-time-${this.id}`);
        const blank = document.getElementById(`vp-blank-${this.id}`);
        const ov    = document.getElementById(`vp-ov-${this.id}`);
        const ovc   = document.getElementById(`vp-ovc-${this.id}`);
        const spd   = document.getElementById(`vp-spd-${this.id}`);

        const fmt = t => {
            if (!isFinite(t) || t < 0) return '0:00';
            const s  = Math.floor(t);
            const m  = Math.floor(s / 60);
            const h  = Math.floor(m / 60);
            const ss = String(s % 60).padStart(2, '0');
            const mm = String(m % 60).padStart(2, '0');
            return h ? `${h}:${mm}:${ss}` : `${m}:${ss}`;
        };

        const updateTime = () => {
            time.textContent = `${fmt(vid.currentTime)} / ${fmt(vid.duration)}`;
            if (vid.duration) {
                const pct = vid.currentTime / vid.duration;
                fill.style.width = (pct * 100) + '%';
                seek.value = Math.round(pct * 1000);
            }
        };

        const updatePlayBtn = () => {
            play.innerHTML = vid.paused ? '&#9654;' : '⏸';
        };

        const flashIcon = icon => {
            ovc.textContent = icon;
            ovc.classList.add('flash');
            clearTimeout(this._flashTimer);
            this._flashTimer = setTimeout(() => ovc.classList.remove('flash'), 450);
        };

        vid.addEventListener('timeupdate',     updateTime);
        vid.addEventListener('durationchange', updateTime);
        vid.addEventListener('play',           updatePlayBtn);
        vid.addEventListener('pause',          updatePlayBtn);
        vid.addEventListener('ended',          updatePlayBtn);
        vid.addEventListener('loadeddata', () => {
            blank.style.display = 'none';
            vid.style.display   = '';
        });
        vid.addEventListener('error', () => {
            if (!vid.src) return;
            const icon = blank.querySelector('.vp-blank-icon');
            const txt  = blank.querySelector('.vp-blank-txt');
            if (icon) icon.textContent = '⚠';
            if (txt)  txt.textContent  = 'Error: cannot load video';
            blank.style.display = '';
            vid.style.display   = 'none';
        });

        const doToggle = () => {
            if (!vid.src) return;
            if (vid.paused) { vid.play().then(() => flashIcon('▶')).catch(() => {}); }
            else            { vid.pause(); flashIcon('⏸'); }
        };
        play.addEventListener('click', doToggle);
        ov.addEventListener('click',   doToggle);

        rew.addEventListener('click', () => { vid.currentTime = Math.max(0, vid.currentTime - 10); });
        fwd.addEventListener('click', () => { vid.currentTime = Math.min(vid.duration || 0, vid.currentTime + 10); });

        seek.addEventListener('input', () => {
            if (vid.duration) {
                const pct = seek.value / 1000;
                vid.currentTime  = pct * vid.duration;
                fill.style.width = (pct * 100) + '%';
            }
        });

        vol.addEventListener('input', () => {
            vid.volume = parseFloat(vol.value);
            vid.muted  = (vid.volume === 0);
            mute.innerHTML = vid.muted ? '&#128263;' : '&#128266;';
        });
        mute.addEventListener('click', () => {
            vid.muted = !vid.muted;
            mute.innerHTML = vid.muted ? '&#128263;' : '&#128266;';
            if (!vid.muted && vid.volume === 0) { vid.volume = 0.5; vol.value = 0.5; }
        });

        spd.addEventListener('change', () => { vid.playbackRate = parseFloat(spd.value); });

        const doLoad = () => {
            const src = addr.value.trim();
            if (src) this._loadSrc(src);
        };
        loadB.addEventListener('click', doLoad);
        addr.addEventListener('keydown', e => { if (e.key === 'Enter') doLoad(); });
    }

    _loadSrc(src) {
        const vid   = document.getElementById(`vp-vid-${this.id}`);
        const blank = document.getElementById(`vp-blank-${this.id}`);
        const addr  = document.getElementById(`vp-addr-${this.id}`);
        const fill  = document.getElementById(`vp-fill-${this.id}`);
        const seek  = document.getElementById(`vp-seek-${this.id}`);
        const time  = document.getElementById(`vp-time-${this.id}`);
        const play  = document.getElementById(`vp-play-${this.id}`);
        if (!vid) return;

        // Resolve vfs:// paths through the virtual filesystem
        let resolved = src;
        if (src.startsWith('vfs://') && typeof VFS !== 'undefined') {
            resolved = VFS.resolveToSrc(src) || src;
        }

        this._src  = src;
        addr.value = src;

        const icon = blank.querySelector('.vp-blank-icon');
        const txt  = blank.querySelector('.vp-blank-txt');
        if (icon) icon.textContent = '⏳';
        if (txt)  txt.textContent  = 'Loading…';
        blank.style.display = '';
        vid.style.display   = 'none';

        fill.style.width = '0%';
        seek.value       = 0;
        time.textContent = '0:00 / 0:00';
        play.innerHTML   = '&#9654;';

        vid.pause();
        vid.src = resolved;
        vid.load();

        const name = src.split('/').filter(Boolean).pop() || 'Video Player';
        this.setTitle(name);
    }

    _esc(s) {
        return String(s || '').replace(/&/g, '&amp;').replace(/"/g, '&quot;');
    }
}
