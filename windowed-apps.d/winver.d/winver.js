'use strict';

class InfoWindow extends AppWindow {
    constructor(opts = {}) {
        super({
            width: 364, height: 280,
            window_name: 'About',
            app_class:   'info',
            icon_name:   'ℹ️',
            resizable:   false,
            ...opts,
        });
    }

    renderContent() {
        const s = Config.system;
        this.contentEl.innerHTML = `
<div class="info-root">
  <div class="info-banner">
    <div class="info-hdr">
      <div class="info-name">Advanced Server</div>
      <div class="info-name">Information Page</div>
      <div class="info-ver">Version ${s.version}</div>
    </div>
  </div>
  <div class="info-body">
    <div class="info-row"><span class="ir-k">Mode</span><span class="ir-v">${Config.vfs.mode}</span></div>
    <div class="info-row"><span class="ir-k">Runtime</span><span class="ir-v">Web Browser</span></div>
    <div class="info-row"><span class="ir-k">Max Windows</span><span class="ir-v">${Config.system.maxWindows}</span></div>
    <div class="info-actions">
      <button class="info-btn reboot-btn" id="info-rb-${this.id}">⏻ Reboot</button>
      <button class="info-btn close-btn"  id="info-cl-${this.id}">Close</button>
    </div>
  </div>
</div>`;

        document.getElementById(`info-rb-${this.id}`)
            .addEventListener('click', () => setTimeout(() => location.reload(), 250));
        document.getElementById(`info-cl-${this.id}`)
            .addEventListener('click', () => this.close());
    }
}
