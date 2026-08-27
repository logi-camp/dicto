import { LitElement, html } from 'lit';

/**
 * A bounded, scrollable text block for the original / translation text.
 * `emphasize` bumps size/weight (used for the translation).
 * Light DOM (no shadow root) so Tailwind classes apply directly.
 */
export class DictoTextBlock extends LitElement {
  static properties = { emphasize: { type: Boolean, reflect: true } };
  createRenderRoot() { return this; }
  render() {
    return html`
      <div
        class="block overflow-y-auto text-ink text-[13px] leading-relaxed
               max-h-30
               [scrollbar-width:thin] [scrollbar-color:var(--color-line)_transparent]
               ${this.emphasize ? 'max-h-70 text-sm font-medium' : ''}"
      ><slot></slot></div>`;
  }
}
customElements.define('dicto-text-block', DictoTextBlock);
