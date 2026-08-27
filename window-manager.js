'use strict';

class AppWindow {
    constructor(opts = {}) {
        this.id          = AppWindow._nextId++;
        this.width       = Math.max(240, Math.min(opts.width  || 800, 65535));
        this.height      = Math.max(180, Math.min(opts.height || 600, 65535));
        this.pos_x       = Math.min(opts.pos_x  ?? 80,  65535);
        this.pos_y       = Math.min(opts.pos_y  ?? 60,  65535);
        this.window_name = opts.window_name || 'Window';
        this.fullscreen  = opts.fullscreen  || false;
        this.app_class   = opts.app_class   || 'generic';
        this.icon_name   = opts.icon_name   || '<img class="ico-img" src="graphical-berries/default.ico" alt="">';
        this.resizable   = opts.resizable !== false;
        this.minimized   = false;
        this._prevGeom   = null;

        this.element = this._buildDOM();
        this._bindDrag();
        this._bindResize();
    }

    _buildDOM() {
        const el = document.createElement('div');
        el.className = 'app-window';
        el.style.cssText = `left:${this.pos_x}px;top:${this.pos_y}px;width:${this.width}px;height:${this.height}px;`;
        el.dataset.windowId = this.id;
        el.dataset.appClass = this.app_class;

        el.innerHTML = `
<div class="wnd-titlebar">
  <div class="wnd-controls">
    <button class="wc-btn wc-close" title="Close">&#10005;</button>
    ${this.resizable ? '<button class="wc-btn wc-max" title="Maximize">&#9633;</button>' : ''}
    <button class="wc-btn wc-min"   title="Minimize">&#8722;</button>
  </div>
  <span class="wnd-title">${this._esc(this.window_name)}</span>
  <span class="wnd-icon">${this.icon_name}</span>
</div>
<div class="wnd-content"></div>
${this.resizable ? `
<div class="rs rs-n"  data-d="n"></div>
<div class="rs rs-ne" data-d="ne"></div>
<div class="rs rs-e"  data-d="e"></div>
<div class="rs rs-se" data-d="se"></div>
<div class="rs rs-s"  data-d="s"></div>
<div class="rs rs-sw" data-d="sw"></div>
<div class="rs rs-w"  data-d="w"></div>
<div class="rs rs-nw" data-d="nw"></div>` : ''}`;

        el.querySelector('.wc-close').addEventListener('click', e => { e.stopPropagation(); this.close(); });
        const maxBtn = el.querySelector('.wc-max');
        if (maxBtn) maxBtn.addEventListener('click', e => { e.stopPropagation(); this.toggleMaximize(); });
        el.querySelector('.wc-min').addEventListener('click',   e => { e.stopPropagation(); this.minimize(); });
        el.querySelector('.wnd-titlebar').addEventListener('dblclick', e => {
            if (!e.target.classList.contains('wc-btn') && this.resizable) this.toggleMaximize();
        });
        return el;
    }

    _esc(s) {
        return String(s).replace(/&/g,'&amp;').replace(/</g,'&lt;').replace(/>/g,'&gt;');
    }

    get contentEl() { return this.element.querySelector('.wnd-content'); }

    _bindDrag() {
        const bar = this.element.querySelector('.wnd-titlebar');
        let on = false, sx, sy, ox, oy;

        bar.addEventListener('mousedown', e => {
            if (e.target.classList.contains('wc-btn')) return;

            if (this.fullscreen) {
                const ratio = window.innerWidth > 0 ? e.clientX / window.innerWidth : 0.5;
                this.toggleMaximize();
                const rw  = parseInt(this.element.style.width) || this.width;
                const nx  = Math.max(0, Math.min(e.clientX - Math.round(rw * ratio), window.innerWidth - rw));
                this.pos_x = nx; this.pos_y = 0;
                this.element.style.left = nx + 'px';
                this.element.style.top  = '0px';
            }

            on = true;
            sx = e.clientX; sy = e.clientY;
            ox = parseInt(this.element.style.left) || 0;
            oy = parseInt(this.element.style.top)  || 0;
            e.preventDefault();
        });

        document.addEventListener('mousemove', e => {
            if (!on) return;
            this.pos_x = Math.max(0, ox + e.clientX - sx);
            this.pos_y = Math.max(0, oy + e.clientY - sy);
            this.element.style.left = this.pos_x + 'px';
            this.element.style.top  = this.pos_y + 'px';
        });
        document.addEventListener('mouseup', () => { on = false; });
    }

    _bindResize() {
        this.element.querySelectorAll('.rs').forEach(h => {
            let on = false, dir, sx, sy, ox, oy, ow, oh;

            h.addEventListener('mousedown', e => {
                if (this.fullscreen) return;
                on = true; dir = h.dataset.d;
                sx = e.clientX; sy = e.clientY;
                ox = parseInt(this.element.style.left)   || 0;
                oy = parseInt(this.element.style.top)    || 0;
                ow = parseInt(this.element.style.width)  || this.width;
                oh = parseInt(this.element.style.height) || this.height;
                e.stopPropagation(); e.preventDefault();
            });
            document.addEventListener('mousemove', e => {
                if (!on) return;
                const dx = e.clientX - sx, dy = e.clientY - sy;
                let nx = ox, ny = oy, nw = ow, nh = oh;
                if (dir.includes('e')) nw = Math.max(240, ow + dx);
                if (dir.includes('s')) nh = Math.max(180, oh + dy);
                if (dir.includes('w')) { nw = Math.max(240, ow - dx); nx = ox + ow - nw; }
                if (dir.includes('n')) { nh = Math.max(180, oh - dy); ny = oy + oh - nh; }
                this.pos_x = nx; this.pos_y = ny; this.width = nw; this.height = nh;
                Object.assign(this.element.style, {
                    left: nx+'px', top: ny+'px', width: nw+'px', height: nh+'px'
                });
                this.onResize?.(nw, nh);
            });
            document.addEventListener('mouseup', () => { on = false; });
        });
    }

    minimize() {
        this.minimized = true;
        this.element.classList.add('minimized');
        WindowManager._updateTaskbar();
    }

    restore() {
        this.minimized = false;
        this.element.classList.remove('minimized');
        WindowManager.focus(this);
        WindowManager._updateTaskbar();
    }

    toggleMaximize() {
        const taskH = parseInt(getComputedStyle(document.documentElement)
            .getPropertyValue('--task-h')) || 36;
        if (this.fullscreen) {
            this.fullscreen = false;
            this.element.classList.remove('fullscreen');
            if (this._prevGeom) {
                const g = this._prevGeom;
                Object.assign(this.element.style,
                    { left: g.x+'px', top: g.y+'px', width: g.w+'px', height: g.h+'px' });
            }
        } else {
            this._prevGeom = {
                x: parseInt(this.element.style.left)   || 0,
                y: parseInt(this.element.style.top)    || 0,
                w: parseInt(this.element.style.width)  || this.width,
                h: parseInt(this.element.style.height) || this.height,
            };
            this.fullscreen = true;
            this.element.classList.add('fullscreen');
            Object.assign(this.element.style, {
                left: '0', top: '0',
                width:  window.innerWidth  + 'px',
                height: (window.innerHeight - taskH) + 'px',
            });
        }
        this.onResize?.(parseInt(this.element.style.width), parseInt(this.element.style.height));
    }

    close() { WindowManager.close(this); }

    setTitle(t) {
        this.window_name = t;
        const el = this.element.querySelector('.wnd-title');
        if (el) el.textContent = t;
        WindowManager._updateTaskbar();
    }

    renderContent() {}
}
AppWindow._nextId = 0;

const WindowManager = {
    windows:     [],
    _z:          100,

    get MAX_WINDOWS() { return Config?.system?.maxWindows ?? 16; },

    open(Cls, opts = {}) {
        if (this.windows.length >= this.MAX_WINDOWS) {
            document.getElementById('max-windows-warning').classList.remove('hidden');
            return null;
        }
        const off  = this.windows.length * 22;
        opts.pos_x = opts.pos_x ?? (80  + off % 360);
        opts.pos_y = opts.pos_y ?? (50  + off % 200);

        const win = new Cls(opts);
        this.windows.push(win);
        document.getElementById('window-layer').appendChild(win.element);
        win.renderContent();
        this.focus(win);
        this._updateTaskbar();
        return win;
    },

    close(win) {
        const i = this.windows.indexOf(win);
        if (i === -1) return;
        this.windows.splice(i, 1);
        win.element.remove();
        this._updateTaskbar();
    },

    focus(win) {
        this._z++;
        win.element.style.zIndex = this._z;
        this.windows.forEach(w => w.element.classList.toggle('focused', w === win));
        if (win.minimized) {
            win.minimized = false;
            win.element.classList.remove('minimized');
        }
        this._updateTaskbar();
    },

    minimizeAll() {
        this.windows.forEach(w => w.minimize());
        this._updateTaskbar();
    },

    _updateTaskbar() {
        if (typeof Desktop !== 'undefined') Desktop.updateTaskbar();
    },
};

document.addEventListener('mousedown', function(e) {
    let node = e.target;
    while (node && node !== document.documentElement) {
        if (node.classList?.contains('app-window')) {
            const win = WindowManager.windows.find(w => w.element === node);
            if (win) WindowManager.focus(win);
            return;
        }
        node = node.parentElement;
    }
}, true);
