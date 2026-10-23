import { LitElement, css, html } from "lit";
import { customElement, state } from "lit/decorators.js";
import type { InstanceDto } from "../types";
import { listInstances, launchInstance } from "../api";
import { showToast } from "../toast";

@customElement("easymc-home-page")
export class HomePage extends LitElement {
  @state() private instances: InstanceDto[] = [];
  @state() private loading = true;

  static styles = css`
    :host {
      display: block;
    }
    .hero {
      display: grid;
      grid-template-columns: 1fr auto;
      gap: 24px;
      align-items: stretch;
      padding: 24px;
      border-radius: var(--md-sys-shape-corner-extra-large);
      background: linear-gradient(
        135deg,
        var(--md-sys-color-primary-container) 0%,
        #e0f2f1 55%,
        var(--md-sys-color-surface-container) 100%
      );
      box-shadow: var(--md-sys-elevation-1);
      margin-bottom: 28px;
    }
    .hero-body {
      display: flex;
      flex-direction: column;
      gap: 10px;
      min-width: 0;
    }
    .thumb {
      width: 96px;
      height: 96px;
      border-radius: var(--md-sys-shape-corner-large);
      background: linear-gradient(160deg, #4db6ac, #00695c);
      display: grid;
      place-items: center;
      font-size: 2.5rem;
      color: white;
      margin-bottom: 8px;
    }
    h1 {
      margin: 0;
      font-size: 1.75rem;
      font-weight: 700;
    }
    .tags {
      display: flex;
      gap: 8px;
      flex-wrap: wrap;
    }
    .tag {
      padding: 4px 12px;
      border-radius: var(--md-sys-shape-corner-full);
      font-size: 0.75rem;
      font-weight: 500;
      background: var(--md-sys-color-surface-container-highest);
      color: var(--md-sys-color-on-surface-variant);
    }
    .tag.accent {
      background: var(--md-sys-color-primary);
      color: var(--md-sys-color-on-primary);
    }
    .desc {
      color: var(--md-sys-color-on-surface-variant);
      margin: 0;
    }
    .meta {
      margin-top: auto;
      padding-top: 12px;
      font-size: 0.85rem;
      color: var(--md-sys-color-on-surface-variant);
    }
    .launch {
      align-self: center;
      display: inline-flex;
      align-items: center;
      gap: 8px;
      padding: 14px 28px;
      border: none;
      border-radius: var(--md-sys-shape-corner-full);
      background: var(--md-sys-color-primary);
      color: var(--md-sys-color-on-primary);
      font-size: 1rem;
      font-weight: 600;
      cursor: pointer;
      box-shadow: var(--md-sys-elevation-2);
    }
    .launch:hover {
      filter: brightness(1.05);
    }
    .launch:active {
      filter: brightness(0.95);
    }
    .section-head {
      display: flex;
      justify-content: space-between;
      align-items: baseline;
      margin-bottom: 14px;
    }
    .section-head h2 {
      margin: 0;
      font-size: 1.15rem;
      font-weight: 600;
    }
    .link {
      color: var(--md-sys-color-primary);
      font-size: 0.9rem;
      cursor: pointer;
      background: none;
      border: none;
      font-weight: 500;
    }
    .grid {
      display: grid;
      grid-template-columns: repeat(auto-fill, minmax(200px, 1fr));
      gap: 16px;
    }
    .card {
      position: relative;
      padding: 16px;
      border-radius: var(--md-sys-shape-corner-large);
      background: var(--md-sys-color-surface-container-low);
      box-shadow: var(--md-sys-elevation-1);
      border: 1px solid transparent;
      min-height: 140px;
      display: flex;
      flex-direction: column;
      gap: 8px;
    }
    .card:nth-child(1) {
      background: #e0f2f1;
    }
    .card:nth-child(2) {
      background: #f5f0e6;
    }
    .card:nth-child(3) {
      background: #e3f2fd;
    }
    .card-icon {
      width: 40px;
      height: 40px;
      border-radius: var(--md-sys-shape-corner-medium);
      background: rgba(255, 255, 255, 0.7);
      display: grid;
      place-items: center;
      font-size: 1.25rem;
    }
    .card h3 {
      margin: 0;
      font-size: 1rem;
    }
    .card .ver {
      font-size: 0.8rem;
      color: var(--md-sys-color-on-surface-variant);
    }
    .status {
      margin-top: auto;
      display: flex;
      align-items: center;
      gap: 6px;
      font-size: 0.8rem;
      color: var(--md-sys-color-success);
      font-weight: 500;
    }
    .dot {
      width: 8px;
      height: 8px;
      border-radius: 50%;
      background: var(--md-sys-color-success);
    }
    .more {
      position: absolute;
      top: 8px;
      right: 8px;
      border: none;
      background: transparent;
      cursor: pointer;
      color: var(--md-sys-color-on-surface-variant);
      font-size: 1.1rem;
      line-height: 1;
      padding: 4px 8px;
      border-radius: var(--md-sys-shape-corner-full);
    }
  `;

  connectedCallback() {
    super.connectedCallback();
    void this.load();
  }

  private async load() {
    this.loading = true;
    this.instances = await listInstances();
    this.loading = false;
  }

  private get featured(): InstanceDto | undefined {
    return this.instances[0];
  }

  private get others(): InstanceDto[] {
    return this.instances.slice(1);
  }

  private async onLaunch(id: string) {
    const msg = await launchInstance(id);
    showToast(msg);
  }

  private icons = ["🌲", "🛡️", "🧊", "⛏️"];

  render() {
    const f = this.featured;
    if (this.loading || !f) {
      return html`<p>加载中…</p>`;
    }
    const ver = typeof f.version === "string" ? f.version : String(f.version);
    return html`
      <section class="hero">
        <div class="hero-body">
          <div class="thumb" aria-hidden="true">☀</div>
          <h1>${f.name}</h1>
          <div class="tags">
            <span class="tag">${ver}</span>
            <span class="tag accent">${f.loader}</span>
          </div>
          <p class="desc">${f.description ?? ""}</p>
          <div class="meta">
            Last played: ${f.last_played ?? "—"} · 42 min played
          </div>
        </div>
        <button class="launch" type="button" @click=${() => this.onLaunch(f.id)}>
          ▶ 启动
        </button>
      </section>

      <div class="section-head">
        <h2>Your Instances</h2>
        <button class="link" type="button">View all</button>
      </div>
      <div class="grid">
        ${this.others.map(
          (inst, i) => html`
            <article class="card">
              <button class="more" type="button" aria-label="更多">⋯</button>
              <div class="card-icon">${this.icons[i % this.icons.length]}</div>
              <h3>${inst.name}</h3>
              <div class="ver">
                ${typeof inst.version === "string" ? inst.version : inst.version}
              </div>
              <div class="status"><span class="dot"></span> Ready</div>
            </article>
          `,
        )}
      </div>
    `;
  }
}

declare global {
  interface HTMLElementTagNameMap {
    "easymc-home-page": HomePage;
  }
}
