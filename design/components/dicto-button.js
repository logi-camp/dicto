import { LitElement, html } from 'lit';

/**
 * Buttons used across the popup. Light DOM (no shadow root) so Tailwind
 * classes apply directly.
 *
 * variant: 'primary' (Translate), 'ghost' (play/pause), 'icon' (square glyph)
 */
export class DictoButton extends LitElement {
  static properties = {
    variant: { type: String },
    disabled: { type: Boolean, reflect: true },
  };
  constructor() {
    super();
    this.variant = 'ghost';
    this.disabled = false;
  }
  createRenderRoot() { return this; }
  render() {
    const base = 'inline-flex items-center gap-1 rounded-md cursor-pointer font-sans transition-[colors,opacity] duration-150 focus-visible:outline focus-visible:outline-1 focus-visible:outline-primary';
    const variants = {
      // the main Translate action
      primary: 'px-4 py-[6px] bg-primary text-bg text-xs font-semibold hover:opacity-90 disabled:opacity-45 disabled:cursor-default',
      // text ghost (Speak)
      ghost: 'px-[9px] py-1 bg-elevated text-muted border border-line text-xs leading-none hover:bg-hover hover:text-ink disabled:opacity-45 disabled:cursor-default',
      // icon-only ghost (▶ ⏸ ↺): square hit area
      icon: 'px-[7px] py-1 bg-elevated text-muted border border-line text-[13px] leading-none hover:bg-hover hover:text-ink disabled:opacity-45 disabled:cursor-default',
    };
    return html`
      <button
        type="button"
        class="${base} ${variants[this.variant] || variants.ghost}"
        ?disabled=${this.disabled}
        @click=${() => this.dispatchEvent(new CustomEvent('action', { bubbles: true, composed: true }))}
      ><slot></slot></button>`;
  }
}
customElements.define('dicto-button', DictoButton);
