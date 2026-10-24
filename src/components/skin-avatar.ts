import { LitElement, css, html } from "lit";
import { customElement, property, state } from "lit/decorators.js";
import type { AccountDto } from "../types";
import { STEVE_AVATAR_URL, skinAvatarUrl } from "../lib/skin-avatar";

type AvatarPhase = "primary" | "steve" | "css";

/**
 * Minecraft skin face (transparent PNG via mc-heads.net).
 * Falls back to classic Steve CDN, then a subtle CSS pixel head.
 */
@customElement("easymc-skin-avatar")
export class SkinAvatar extends LitElement {
  @property({ attribute: false }) account: Pick<AccountDto, "kind" | "username" | "uuid"> | null =
    null;
  @property({ type: Number }) size = 64;
  @state() private phase: AvatarPhase = "primary";

  static styles = css`
    :host {
      display: inline-grid;
      place-items: center;
      width: var(--easymc-skin-avatar-size, 36px);
      height: var(--easymc-skin-avatar-size, 36px);
      flex-shrink: 0;
      border-radius: 8px;
      overflow: hidden;
      background: transparent;
      image-rendering: pixelated;
    }
    img {
      width: 100%;
      height: 100%;
      object-fit: cover;
      display: block;
      background: transparent;
      image-rendering: pixelated;
    }
    .ph {
      width: 100%;
      height: 100%;
      display: block;
      position: relative;
      background: linear-gradient(
        180deg,
        oklch(0.78 0.04 70) 0 44%,
        oklch(0.55 0.06 200) 44% 100%
      );
    }
    .ph::before {
      content: "";
      position: absolute;
      inset: 22% 18% 38% 18%;
      background: linear-gradient(
          90deg,
          transparent 0 18%,
          oklch(0.22 0.02 250) 18% 32%,
          transparent 32% 68%,
          oklch(0.22 0.02 250) 68% 82%,
          transparent 82%
        ),
        oklch(0.86 0.04 70);
    }
    .ph::after {
      content: "";
      position: absolute;
      left: 30%;
      right: 30%;
      top: 58%;
      height: 10%;
      background: oklch(0.35 0.06 30);
    }
  `;

  updated(changed: Map<PropertyKey, unknown>) {
    if (changed.has("account") || changed.has("size")) {
      this.phase = "primary";
    }
  }

  private get src(): string {
    if (this.phase === "steve") return STEVE_AVATAR_URL;
    return skinAvatarUrl(this.account, this.size) ?? STEVE_AVATAR_URL;
  }

  private onError = () => {
    if (this.phase === "primary") {
      const primary = skinAvatarUrl(this.account, this.size);
      // If we already pointed at Steve (no identity), skip straight to CSS.
      if (!primary || primary === STEVE_AVATAR_URL) {
        this.phase = "css";
      } else {
        this.phase = "steve";
      }
      return;
    }
    this.phase = "css";
  };

  render() {
    if (this.phase === "css") {
      return html`<span class="ph" aria-hidden="true"></span>`;
    }
    return html`
      <img
        src=${this.src}
        alt=""
        width=${this.size}
        height=${this.size}
        decoding="async"
        referrerpolicy="no-referrer"
        @error=${this.onError}
      />
    `;
  }
}

declare global {
  interface HTMLElementTagNameMap {
    "easymc-skin-avatar": SkinAvatar;
  }
}
