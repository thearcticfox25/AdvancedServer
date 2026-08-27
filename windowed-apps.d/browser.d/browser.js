'use strict';

class BrowserWindow extends AppWindow {
    constructor(opts = {}) {
        super({
            width: 920, height: 660,
            window_name: opts.window_name || 'Browser',
            app_class:   'browser',
            icon_name:   '<img class="ico-img" src="graphical-berries/default.ico" alt="">',
            ...opts,
        });
        this._vfsUrl = opts.url
            ? (opts.url.startsWith('vfs://') ? opts.url : VFS.srcToVfsUrl(opts.url))
            : '';
    }

    getCurrentAddress() { return this._vfsUrl || '(blank)'; }

    renderContent() {
        const safeUrl  = this._esc(this._vfsUrl);
        const src      = this._vfsUrl ? (VFS.resolveToSrc(this._vfsUrl) || '') : '';

        this.contentEl.innerHTML = `
<div class="br-root">
  <div class="br-bar">
    <button class="br-btn br-go"   id="br-go-${this.id}"   title="Navigate">&#8592;</button>
    <input  class="br-addr"        id="br-addr-${this.id}" type="text"
            value="${safeUrl}" placeholder="vfs://path/to/page" spellcheck="false">
    <button class="br-btn br-fwd"  id="br-fwd-${this.id}"  title="Forward">&lt;</button>
    <button class="br-btn br-back" id="br-back-${this.id}" title="Back">&gt;</button>
    <button class="br-btn br-ext"  id="br-ext-${this.id}"  title="Open in browser" style="${src ? '' : 'opacity:.35;pointer-events:none'}">&#8598;</button>
  </div>
  <div class="br-viewport">
    <iframe class="br-frame" id="br-frame-${this.id}"
            src="${this._esc(src)}"
            sandbox="allow-scripts allow-same-origin allow-forms allow-popups"
            style="${src ? '' : 'display:none'}"></iframe>
    <div class="br-blank" id="br-blank-${this.id}" style="${src ? 'display:none' : ''}">
      <div class="br-blank-icon"><img class="ico-img" src="graphical-berries/default.ico" alt=""></div>
      <div class="br-blank-txt">Enter a <code>vfs://</code> path and press Return or ←</div>
    </div>
  </div>
</div>`;

        this._setupEvents();
        if (this._vfsUrl) this.setTitle(this._displayName(this._vfsUrl));
    }

    _setupEvents() {
        const addr  = document.getElementById(`br-addr-${this.id}`);
        const frame = document.getElementById(`br-frame-${this.id}`);
        const blank = document.getElementById(`br-blank-${this.id}`);
        const ext   = document.getElementById(`br-ext-${this.id}`);

        const setExtState = src => {
            if (!ext) return;
            if (src) { ext.style.opacity = ''; ext.style.pointerEvents = ''; }
            else     { ext.style.opacity = '0.35'; ext.style.pointerEvents = 'none'; }
        };

        const nav = rawInput => {
            let url = rawInput.trim();
            if (url && !url.startsWith('vfs://') && !url.startsWith('http')) {
                url = 'vfs://' + url;
            }
            const src = VFS.resolveToSrc(url);
            if (!src) {
                addr.style.borderColor = 'var(--c-close)';
                setTimeout(() => (addr.style.borderColor = ''), 1000);
                return;
            }
            this._vfsUrl    = url;
            addr.value      = url;
            addr.style.borderColor = '';
            frame.src       = src;
            frame.style.display = '';
            blank.style.display = 'none';
            this.setTitle(this._displayName(url));
            setExtState(src);
        };

        document.getElementById(`br-go-${this.id}`)
            .addEventListener('click', () => nav(addr.value));
        addr.addEventListener('keydown', e => { if (e.key === 'Enter') nav(addr.value); });

        document.getElementById(`br-back-${this.id}`)
            .addEventListener('click', () => { try { frame.contentWindow.history.back(); } catch {} });
        document.getElementById(`br-fwd-${this.id}`)
            .addEventListener('click', () => { try { frame.contentWindow.history.forward(); } catch {} });

        if (ext) ext.addEventListener('click', () => {
            const src = frame.src;
            if (src) window.open(src, '_blank');
        });
    }

    navigate(vfsUrl) {
        const addr  = document.getElementById(`br-addr-${this.id}`);
        const frame = document.getElementById(`br-frame-${this.id}`);
        const blank = document.getElementById(`br-blank-${this.id}`);
        const src   = VFS.resolveToSrc(vfsUrl);
        if (!src) return;
        this._vfsUrl   = vfsUrl;
        if (addr)  addr.value = vfsUrl;
        if (frame) { frame.src = src; frame.style.display = ''; }
        if (blank) blank.style.display = 'none';
        this.setTitle(this._displayName(vfsUrl));
    }

    _displayName(vfsUrl) {
        const rel = VFS.fromVfsUrl(vfsUrl);
        return rel.split('/').filter(Boolean).pop() || 'Browser';
    }

    _esc(s) {
        return String(s || '').replace(/&/g,'&amp;').replace(/"/g,'&quot;');
    }
}
