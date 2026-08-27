import { LitElement, html } from 'lit';
import './dicto-chip.js';

/**
 * The inline Options panel: compact chip selectors for
 * Provider → Model, Target language, TTS preset → Voice.
 * Two-level: choosing a provider/TTS preset swaps the model/voice chip sets.
 * Light DOM (no shadow root) so Tailwind classes apply directly.
 */
export class DictoOptions extends LitElement {
  static properties = {
    open: { type: Boolean, reflect: true },
    provider: { type: String },   // anthropic | openai
    model: { type: String },
    target: { type: String },
    tts: { type: String },        // grok | kokoro | openai
    voice: { type: String },
  };
  constructor() {
    super();
    this.open = false;
    this.provider = 'openai';
    this.model = 'GLM-4.7';
    this.target = 'English';
    this.tts = 'grok';
    this.voice = 'Rex';
  }
  static models = {
    anthropic: ['claude-sonnet-4-6', 'claude-opus-4-6', 'claude-haiku-4-5'],
    openai: ['GLM-4.7', 'gpt-4o-mini', 'llama-3.3-70b'],
  };
  static targets = ['English', 'Persian', 'Spanish', 'French', 'German', 'Arabic', 'Custom…'];
  static ttsPresets = [
    { id: 'grok', label: 'Grok Voice', voices: ['Eve', 'Rex', 'Ara', 'Sal', 'Leo'] },
    { id: 'kokoro', label: 'Kokoro', voices: ['bf_emma', 'af_sky', 'am_adam'] },
    { id: 'openai', label: 'OpenAI', voices: ['alloy', 'nova', 'shimmer', 'onyx'] },
  ];
  createRenderRoot() { return this; }
  chips(items, selected, key) {
    return items.map(
      (item) => html`
        <dicto-chip
          label=${item}
          ?selected=${item === selected}
          data-key=${key}
          data-value=${item}
        ></dicto-chip>`
    );
  }
  onChip(e) {
    const chip = e.composedPath()[0];
    const { key, value } = chip.dataset;
    if (key === 'provider') {
      this.provider = value;
      this.model = DictoOptions.models[this.providerKey(value)][0];
    } else if (key === 'tts') {
      const preset = DictoOptions.ttsPresets.find((p) => p.label === value);
      this.tts = preset.id;
      this.voice = preset.voices[0];
    } else {
      this[key] = value;
    }
  }
  providerKey(label) {
    return label === 'Anthropic' ? 'anthropic' : 'openai';
  }
  render() {
    const preset = DictoOptions.ttsPresets.find((p) => p.id === this.tts);
    const field = (label, chips) => html`
      <span class="text-[10px] tracking-wider uppercase text-muted pt-1 whitespace-nowrap">${label}</span>
      <div class="flex flex-wrap gap-1">${chips}</div>`;
    return html`
      <div
        class="grid grid-cols-[auto_1fr] gap-x-3 gap-y-2 items-start p-2.5 mx-3.5 mb-3
               bg-bg border border-line rounded-md"
        @select=${this.onChip}
      >
        ${field('Provider', this.chips(['Anthropic', 'OpenAI'], this.provider === 'anthropic' ? 'Anthropic' : 'OpenAI', 'provider'))}
        ${field('Model', this.chips(DictoOptions.models[this.provider], this.model, 'model'))}
        ${field('Target', this.chips(DictoOptions.targets, this.target, 'target'))}
        ${field('TTS', DictoOptions.ttsPresets.map(
          (p) => html`<dicto-chip label=${p.label} ?selected=${p.id === this.tts} data-key="tts" data-value=${p.label}></dicto-chip>`
        ))}
        ${field('Voice', this.chips(preset.voices, this.voice, 'voice'))}
      </div>`;
  }
}
customElements.define('dicto-options', DictoOptions);
