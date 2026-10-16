<script lang="ts">
  import {
    THEMES, applyAppearance, loadAppearance, saveAppearance,
    type Appearance, type ContentWidth, type Density, type Theme,
  } from '../theme'
  import { onMount } from 'svelte'
  import { t } from '../i18n/index.svelte'
  import { api, errText } from '../api'
  import {
    CODE_SIZE, DEFAULT_FONTS, TERMINAL_SIZE, TEXT_SIZE, applyFonts, loadFonts, normalizeFonts, saveFonts, terminalFontFamily,
    type FontSettings,
  } from '../fonts'

  // Como a aba UI do gitHydra: cada tema é um cartão com uma mini janela
  // pintada nas cores dele. Mais "Sistema" e as opções de layout.
  let a = $state<Appearance>(loadAppearance())

  function set<K extends keyof Appearance>(k: K, v: Appearance[K]) {
    a = { ...a, [k]: v }
    saveAppearance(a)
    applyAppearance(a)
  }

  const dark = THEMES.filter((th) => th.mode === 'dark')
  const light = THEMES.filter((th) => th.mode === 'light')
  const system = { id: 'system', get label() { return t('appearance.system') }, mode: 'dark', palette: THEMES[0].palette } as Theme

  const DENSITY: { id: Density; labelKey: string; hintKey: string }[] = [
    { id: 'compact', labelKey: 'appearance.densityCompact', hintKey: 'appearance.densityCompactHint' },
    { id: 'normal', labelKey: 'appearance.densityNormal', hintKey: '' },
    { id: 'comfortable', labelKey: 'appearance.densityComfortable', hintKey: 'appearance.densityComfortableHint' },
  ]
  const WIDTH: { id: ContentWidth; labelKey: string }[] = [
    { id: 'narrow', labelKey: 'appearance.widthNarrow' },
    { id: 'normal', labelKey: 'appearance.widthNormal' },
    { id: 'wide', labelKey: 'appearance.widthWide' },
    { id: 'full', labelKey: 'appearance.widthFull' },
  ]
  const SCALES = [90, 100, 110, 125]

  // ---- fontes (como em Settings > UI do gitHydra) ----
  let fonts = $state<FontSettings>(loadFonts())
  let sys = $state<{ families: string[]; defaultText: string; defaultCode: string } | null>(null)
  let fontsError = $state('')

  onMount(() => {
    api.listSystemFonts().then(
      (r) => (sys = r),
      (e) => (fontsError = errText(e)),
    )
  })

  function setFonts(patch: Partial<FontSettings>) {
    fonts = normalizeFonts({ ...fonts, ...patch })
    saveFonts(fonts)
    applyFonts(fonts)
  }

  /** O que o padrão do app (pilhas --sans/--mono do app.css) vira nesta máquina. */
  function defaultName(mono: boolean) {
    if (!sys) return ''
    const preferred = mono ? ['JetBrains Mono', 'Fira Code', 'SF Mono'] : ['Inter']
    return preferred.find((f) => sys!.families.includes(f)) ?? (mono ? sys.defaultCode : sys.defaultText)
  }

  function familyOptions(current: string) {
    const installed = sys?.families ?? []
    return current && !installed.includes(current) ? [current, ...installed] : installed
  }

  function onSize(key: 'textSize' | 'codeSize' | 'terminalSize', range: { min: number; max: number }, e: Event) {
    const n = (e.currentTarget as HTMLInputElement).valueAsNumber
    if (n >= range.min && n <= range.max) setFonts({ [key]: n })
  }
</script>

{#snippet preview(th: Theme)}
  {@const c = th.palette}
  <div class="pv" style="background:{c.bg}; border-color:{c.borderStrong}">
    <div class="pv-side" style="background:{c.panel}; border-color:{c.border}">
      <i style="background:{c.faint}"></i>
      <i style="background:{c.active}; width:80%"></i>
      <i style="background:{c.faint}; width:60%"></i>
    </div>
    <div class="pv-main">
      <i style="background:{c.text}; width:80%"></i>
      <i style="background:{c.muted}; width:60%"></i>
      <i style="background:{c.muted}; width:68%"></i>
      <span class="pv-row">
        <i style="background:{c.accent}; width:24%"></i>
        <i style="background:{c.blue}; width:14%"></i>
        <i style="background:{c.red}; width:10%"></i>
      </span>
    </div>
  </div>
{/snippet}

{#snippet card(th: Theme, label: string, isSystem = false)}
  <button class="card" class:on={a.theme === th.id} aria-pressed={a.theme === th.id} onclick={() => set('theme', th.id)}>
    {#if isSystem}
      <div class="split">
        <div class="half">{@render preview(THEMES.find((x) => x.id === 'dark')!)}</div>
        <div class="half right">{@render preview(THEMES.find((x) => x.id === 'light')!)}</div>
      </div>
    {:else}
      {@render preview(th)}
    {/if}
    <span class="name">{label}</span>
  </button>
{/snippet}

<h4>{t('appearance.theme')}</h4>
<div class="grid">
  {@render card(system, t('appearance.system'), true)}
</div>
<div class="sub">{t('appearance.dark')}</div>
<div class="grid">
  {#each dark as th (th.id)}{@render card(th, th.label)}{/each}
</div>
<div class="sub">{t('appearance.light')}</div>
<div class="grid">
  {#each light as th (th.id)}{@render card(th, th.label)}{/each}
</div>

<h4>{t('appearance.layout')}</h4>
<div class="opts">
  <span class="lbl">{t('appearance.treeDensity')}</span>
  <div class="seg">
    {#each DENSITY as d (d.id)}
      <button class:on={a.density === d.id} title={d.hintKey ? t(d.hintKey) : ''} onclick={() => set('density', d.id)}>{t(d.labelKey)}</button>
    {/each}
  </div>

  <span class="lbl">{t('appearance.chatWidth')}</span>
  <div class="seg">
    {#each WIDTH as w (w.id)}
      <button class:on={a.contentWidth === w.id} onclick={() => set('contentWidth', w.id)}>{t(w.labelKey)}</button>
    {/each}
  </div>

  <span class="lbl">{t('appearance.uiSize')}</span>
  <div class="seg">
    {#each SCALES as s (s)}
      <button class:on={a.scale === s} onclick={() => set('scale', s)}>{s}%</button>
    {/each}
  </div>
</div>

<h4>{t('appearance.fonts')}</h4>
{#snippet fontRow(label: string, family: 'textFamily' | 'codeFamily' | 'terminalFamily', size: 'textSize' | 'codeSize' | 'terminalSize', range: { min: number; max: number }, mono: boolean, defaultLabel?: string)}
  {@const def = defaultLabel ?? defaultName(mono)}
  <span class="lbl">{label}</span>
  <div class="font-row">
    <select class="font-sel" value={fonts[family]} onchange={(e) => setFonts({ [family]: e.currentTarget.value })}>
      <option value="">{defaultLabel ?? (def ? t('appearance.fontDefaultNamed', { name: def }) : t('appearance.fontDefault'))}</option>
      {#each familyOptions(fonts[family]) as f (f)}
        <option value={f}>{f}</option>
      {/each}
    </select>
    <input
      class="font-size"
      type="number"
      min={range.min}
      max={range.max}
      value={fonts[size]}
      title={t('appearance.fontSize')}
      aria-label={t('appearance.fontSize')}
      oninput={(e) => onSize(size, range, e)}
    />
    <span class="px">px</span>
  </div>
{/snippet}
<div class="opts">
  {@render fontRow(t('appearance.textFont'), 'textFamily', 'textSize', TEXT_SIZE, false)}
  <span></span>
  <p class="sample">{t('appearance.textSample')}</p>

  {@render fontRow(t('appearance.codeFont'), 'codeFamily', 'codeSize', CODE_SIZE, true)}
  <span></span>
  <p class="sample mono">const sha = 'a1b2c3d'; // 0O 1lI</p>

  {@render fontRow(t('appearance.terminalFont'), 'terminalFamily', 'terminalSize', TERMINAL_SIZE, true, t('appearance.terminalFontAuto'))}
  <span></span>
  <p class="sample term" style:font-family={terminalFontFamily(fonts)} style:font-size="{fonts.terminalSize}px">{'~/repo \ue0a0 main \uf418 \u276f git status'}</p>

  <span></span>
  <div class="font-foot">
    {#if fontsError || sys?.families.length === 0}
      <p class="warn">{t('appearance.fontsListError', { error: fontsError || t('appearance.fontsNone') })}</p>
    {/if}
    <p class="hint">{t('appearance.fontsHint')}</p>
    <button class="btn" onclick={() => setFonts(DEFAULT_FONTS)}>{t('appearance.resetFonts')}</button>
  </div>
</div>

<style>
  h4 {
    margin: 0 0 10px;
    font-size: 12px;
    text-transform: uppercase;
    letter-spacing: 0.6px;
    color: var(--text-muted);
  }
  h4:not(:first-child) {
    margin-top: 22px;
  }
  .sub {
    font-size: 11.5px;
    color: var(--text-faint);
    margin: 12px 0 6px;
  }
  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(124px, 1fr));
    gap: 10px;
  }
  .card {
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 6px;
    border: 1px solid var(--border);
    border-radius: var(--radius);
    background: var(--bg-panel);
    color: var(--text-muted);
    text-align: left;
  }
  .card:hover {
    border-color: var(--border-strong);
    background: var(--bg-hover);
  }
  .card.on {
    border-color: var(--accent);
    box-shadow: 0 0 0 1px var(--accent);
    color: var(--text);
  }
  .name {
    font-size: 12px;
    padding: 0 2px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .pv {
    display: flex;
    height: 58px;
    border: 1px solid;
    border-radius: 5px;
    overflow: hidden;
  }
  .pv-side {
    width: 28%;
    border-right: 1px solid;
    display: flex;
    flex-direction: column;
    gap: 5px;
    padding: 8px 5px;
  }
  .pv-main {
    flex: 1;
    display: flex;
    flex-direction: column;
    justify-content: center;
    gap: 5px;
    padding: 0 8px;
  }
  .pv i {
    display: block;
    height: 4px;
    border-radius: 3px;
    width: 100%;
  }
  .pv-row {
    display: flex;
    gap: 4px;
    margin-top: 2px;
  }
  .split {
    display: flex;
    height: 60px;
    border-radius: 5px;
    overflow: hidden;
  }
  .half {
    width: 50%;
    overflow: hidden;
  }
  .half :global(.pv) {
    width: 200%;
    border-radius: 0;
  }
  .half.right :global(.pv) {
    margin-left: -100%;
  }
  .opts {
    display: grid;
    grid-template-columns: 160px 1fr;
    gap: 10px 14px;
    align-items: center;
  }
  .lbl {
    color: var(--text-muted);
  }
  .seg {
    display: inline-flex;
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
    overflow: hidden;
    justify-self: start;
  }
  .seg button {
    border: none;
    background: var(--bg-input);
    color: var(--text-muted);
    padding: 5px 12px;
    font-size: 12.5px;
  }
  .seg button + button {
    border-left: 1px solid var(--border);
  }
  .seg button:hover {
    color: var(--text);
  }
  .font-row {
    display: flex;
    align-items: center;
    gap: 6px;
    min-width: 0;
  }
  .font-sel,
  .font-size {
    height: 28px;
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
    background: var(--bg-input);
    color: var(--text);
    padding: 0 8px;
    font-size: 12.5px;
  }
  .font-sel {
    flex: 1;
    min-width: 0;
    max-width: 320px;
  }
  .font-sel option {
    background: var(--bg-elev);
    color: var(--text);
  }
  .font-size {
    width: 64px;
  }
  .font-sel:focus,
  .font-size:focus {
    outline: none;
    border-color: var(--accent);
  }
  .px {
    font-size: 12px;
    color: var(--text-faint);
  }
  .sample {
    margin: -4px 0 4px;
    color: var(--text-muted);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .font-foot {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 8px;
  }
  .font-foot p {
    margin: 0;
    font-size: 12px;
  }
  .hint {
    color: var(--text-faint);
  }
  .warn {
    color: var(--amber);
  }
  .seg button.on {
    background: var(--accent-soft);
    color: var(--text);
    font-weight: 500;
  }
</style>
