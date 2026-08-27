import { LitElement, html } from 'lit';

/**
 * A selectable chip used in the Options panel (provider/model/lang/voice
 * selectors). Light DOM (no shadow root) so Tailwind classes apply directly.
 */
export class DictoChip extends LitElement {
  static properties = {
    label: { type: String },
    selected: { type: Boolean, reflect: true },
  };
  constructor() {
    super();
    this.label = '';
    this.selected = false;
  }
  // Light DOM: render into the element itself, not a shadow root, so this
  // page's global Tailwind stylesheet styles the content.
  createRenderRoot() { return this; }
  render() {
    return html`
      <button
        type="button"
        class="px-2 py-[3px] rounded text-[11px] leading-normal whitespace-nowrap cursor-pointer transition-colors
               ${this.selected
                 ? 'bg-primary text-bg font-semibold'
                 : 'bg-elevated text-muted hover:bg-hover hover:text-ink'}"
        @click=${() => this.dispatchEvent(new CustomEvent('select', { bubbles: true, composed: true }))}
      >${this.label}</button>`;
  }
}
customElements.define('dicto-chip', DictoChip);
