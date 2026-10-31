import { LitElement, css, html } from "lit";
import { customElement, property, query, state } from "lit/decorators.js";
import { SkinViewer, WalkingAnimation } from "skinview3d";

/**
 * 3D Minecraft skin + cape preview (skinview3d / MIT, Blessing Skin community).
 * Accepts http(s) URLs or data: URLs for skin and optional cape.
 */
@customElement("easymc-skin-viewer")
export class EasyMcSkinViewer extends LitElement {
  @property({ type: String }) skin: string | null = null;
  @property({ type: String }) cape: string | null = null;
  /** "default" | "slim" | "auto-detect" */
  @property({ type: String }) model: string = "auto-detect";
  @property({ type: Number }) width = 260;
  @property({ type: Number }) height = 360;
  @property({ type: Boolean }) walk = true;

  @query("canvas") private canvas!: HTMLCanvasElement;
  @state() private err = "";

  private viewer: SkinViewer | null = null;

  static styles = css`
    :host {
      display: block;
      position: relative;
      width: var(--easymc-skin-viewer-w, 260px);
      height: var(--easymc-skin-viewer-h, 360px);
      border-radius: var(--md-sys-shape-corner-large);
      overflow: hidden;
      background:
        radial-gradient(
          ellipse at 50% 70%,
          color-mix(in oklch, var(--md-sys-color-primary) 18%, transparent),
          transparent 55%
        ),
        var(--md-sys-color-surface-container);
      box-shadow: var(--md-sys-elevation-1);
    }
    canvas {
      display: block;
      width: 100%;
      height: 100%;
      touch-action: none;
      cursor: grab;
    }
    canvas:active {
      cursor: grabbing;
    }
    .err {
      position: absolute;
      inset: 0;
      display: grid;
      place-items: center;
      padding: 16px;
      text-align: center;
      font-size: var(--easymc-type-meta);
      color: var(--md-sys-color-on-surface-variant);
      background: var(--md-sys-color-surface-container);
    }
  `;

  firstUpdated() {
    this.initViewer();
  }

  updated(changed: Map<PropertyKey, unknown>) {
    if (!this.viewer) return;
    if (changed.has("width") || changed.has("height")) {
      this.viewer.width = this.width;
      this.viewer.height = this.height;
    }
    if (changed.has("skin") || changed.has("model")) {
      void this.loadSkin();
    }
    if (changed.has("cape")) {
      void this.loadCape();
    }
    if (changed.has("walk")) {
      this.syncAnim();
    }
  }

  disconnectedCallback() {
    this.viewer?.dispose();
    this.viewer = null;
    super.disconnectedCallback();
  }

  private initViewer() {
    try {
      this.viewer?.dispose();
      this.viewer = new SkinViewer({
        canvas: this.canvas,
        width: this.width,
        height: this.height,
      });
      // Transparent / clear backdrop — host CSS paints the card.
      this.viewer.renderer.setClearColor(0x000000, 0);
      this.viewer.controls.enablePan = false;
      this.viewer.zoom = 0.9;
      this.syncAnim();
      void this.loadSkin();
      void this.loadCape();
      this.err = "";
    } catch (e) {
      this.err = "三维预览暂时不可用";
      console.warn("skinview3d init failed", e);
    }
  }

  private syncAnim() {
    if (!this.viewer) return;
    if (this.walk) {
      this.viewer.animation = new WalkingAnimation();
      this.viewer.animation.speed = 0.6;
    } else {
      this.viewer.animation = null;
    }
  }

  private async loadSkin() {
    if (!this.viewer) return;
    if (!this.skin) {
      this.viewer.playerObject.skin.visible = false;
      return;
    }
    try {
      const model =
        this.model === "slim" || this.model === "default"
          ? (this.model as "slim" | "default")
          : "auto-detect";
      await this.viewer.loadSkin(this.skin, { model });
      this.viewer.playerObject.skin.visible = true;
      this.err = "";
    } catch (e) {
      this.err = "皮肤加载失败，换一张再试";
      console.warn("loadSkin", e);
    }
  }

  private async loadCape() {
    if (!this.viewer) return;
    if (!this.cape) {
      this.viewer.loadCape(null);
      return;
    }
    try {
      await this.viewer.loadCape(this.cape, { backEquipment: "cape" });
    } catch (e) {
      console.warn("loadCape", e);
    }
  }

  render() {
    return html`
      <canvas
        width=${this.width}
        height=${this.height}
        aria-label="皮肤三维预览，可拖动旋转"
      ></canvas>
      ${this.err ? html`<div class="err" role="status">${this.err}</div>` : null}
    `;
  }
}

declare global {
  interface HTMLElementTagNameMap {
    "easymc-skin-viewer": EasyMcSkinViewer;
  }
}
