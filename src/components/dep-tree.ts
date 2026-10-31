import { LitElement, css, html, nothing } from "lit";
import { customElement, property } from "lit/decorators.js";
import type { DepNodeDto, DepStatusId } from "../types";

function statusLabel(s: DepStatusId): string {
  switch (s) {
    case "ok":
      return "已满足";
    case "missing":
      return "缺失";
    case "conflict":
      return "冲突";
    case "provided":
      return "环境";
    case "disabled":
      return "已禁用";
    default:
      return "未知";
  }
}

@customElement("easymc-dep-tree")
export class DepTree extends LitElement {
  @property({ attribute: false }) roots: DepNodeDto[] = [];
  @property({ type: Boolean }) compact = false;

  static styles = css`
    :host {
      display: block;
      color: var(--md-sys-color-on-surface);
      font-size: var(--easymc-type-body, 0.875rem);
    }
    ul {
      list-style: none;
      margin: 0;
      padding: 0;
    }
    ul ul {
      margin-left: 1.1rem;
      padding-left: 0.75rem;
      border-left: 2px solid var(--md-sys-color-outline-variant);
    }
    .node {
      display: flex;
      flex-wrap: wrap;
      align-items: baseline;
      gap: 6px 10px;
      padding: 6px 0;
      min-height: 32px;
    }
    .name {
      font-weight: var(--easymc-weight-medium, 500);
      letter-spacing: -0.01em;
    }
    .ver {
      color: var(--md-sys-color-on-surface-variant);
      font-size: var(--easymc-type-meta, 0.75rem);
      font-variant-numeric: tabular-nums;
    }
    .badge {
      display: inline-flex;
      align-items: center;
      padding: 1px 8px;
      border-radius: var(--md-sys-shape-corner-full, 999px);
      font-size: 0.7rem;
      font-weight: 500;
      line-height: 1.5;
    }
    .badge.ok {
      background: var(--md-sys-color-success-container);
      color: var(--md-sys-color-on-success-container);
    }
    .badge.missing {
      background: var(--md-sys-color-error-container);
      color: var(--md-sys-color-on-error-container);
    }
    .badge.conflict {
      background: var(--md-sys-color-warning-container);
      color: var(--md-sys-color-on-warning-container);
    }
    .badge.provided {
      background: var(--md-sys-color-secondary-container);
      color: var(--md-sys-color-on-secondary-container);
    }
    .badge.disabled {
      background: var(--md-sys-color-surface-container-high);
      color: var(--md-sys-color-on-surface-variant);
    }
    .badge.unknown {
      background: var(--md-sys-color-surface-container);
      color: var(--md-sys-color-on-surface-variant);
    }
    .msg {
      flex: 1 1 100%;
      margin: 0;
      color: var(--md-sys-color-on-surface-variant);
      font-size: var(--easymc-type-meta, 0.75rem);
      line-height: 1.4;
    }
    .msg.conflict,
    .msg.missing {
      color: var(--md-sys-color-on-error-container);
    }
    .kind {
      color: var(--md-sys-color-outline);
      font-size: 0.7rem;
    }
    .empty {
      color: var(--md-sys-color-on-surface-variant);
      padding: 8px 0;
    }
  `;

  render() {
    if (!this.roots.length) {
      return html`<p class="empty">暂无依赖树数据</p>`;
    }
    return html`<ul role="tree">${this.roots.map((n) => this.renderNode(n, true))}</ul>`;
  }

  private renderNode(node: DepNodeDto, isRoot: boolean) {
    const showProvided = !this.compact || node.status !== "provided";
    if (!showProvided && !isRoot) return nothing;
    const children = this.compact
      ? node.children.filter((c) => c.status !== "provided")
      : node.children;
    return html`<li role="treeitem">
      <div class="node">
        <span class="name">${node.display_name}</span>
        ${node.version ? html`<span class="ver">${node.version}</span>` : nothing}
        ${node.dep_kind
          ? html`<span class="kind">${node.dep_kind === "required" ? "必需" : node.dep_kind === "optional" ? "可选" : "不兼容"}</span>`
          : nothing}
        <span class="badge ${node.status}">${statusLabel(node.status)}</span>
        ${node.message_zh && node.status !== "ok" && node.status !== "provided"
          ? html`<p class="msg ${node.status}">${node.message_zh}</p>`
          : nothing}
      </div>
      ${children.length
        ? html`<ul role="group">${children.map((c) => this.renderNode(c, false))}</ul>`
        : nothing}
    </li>`;
  }
}

declare global {
  interface HTMLElementTagNameMap {
    "easymc-dep-tree": DepTree;
  }
}
