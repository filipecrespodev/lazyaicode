<script lang="ts">
  import { api, errText } from '../api'
  import { app } from '../stores/app.svelte'
  import { toolSummary, type Item } from '../transcript'
  import Icon from './Icon.svelte'
  import { t } from '../i18n/index.svelte'

  let { item, threadId }: { item: Extract<Item, { kind: 'permission' }>; threadId: string } = $props()
  let busy = $state(false)
  let showInput = $state(false)

  // "Sempre permitir": usa as regras que o próprio CLI sugere (ex.: `Bash(npm test:*)`
  // gravada nas settings locais); sem sugestão, libera a ferramenta só nesta sessão.
  // No ACP, escolhe a opção allow_always do agente.
  const always = $derived.by((): unknown[] | null => {
    if (item.options) return item.options.some((o) => o.kind === 'allow_always') ? [] : null
    return item.suggestions ?? [{ type: 'addRules', rules: [{ toolName: item.toolName }], behavior: 'allow', destination: 'session' }]
  })
  const alwaysHint = $derived.by(() => {
    if (!always?.length) return t('permission.dontAskAgain')
    const rules = always.flatMap((u: any) =>
      u?.type === 'addRules' ? (u.rules ?? []).map((r: any) => (r.ruleContent ? `${r.toolName}(${r.ruleContent})` : r.toolName)) : [],
    )
    const where = (always as any[]).some((u) => u?.destination && u.destination !== 'session') ? t('permission.inSettings') : t('permission.inSession')
    return rules.length ? t('permission.dontAskAgainWhere', { where, rules: rules.join(', ') }) : t('permission.dontAskAgain')
  })

  async function decide(allow: boolean, forever = false) {
    busy = true
    try {
      await api.respondPermission(threadId, item.requestId, allow, item.input, item.toolUseId, undefined, forever && always ? always : undefined)
      item.decided = allow ? (forever ? 'always' : 'allow') : 'deny'
    } catch (e) {
      app.toast(errText(e), 'error')
    } finally {
      busy = false
    }
  }
</script>

<div class="perm" class:done={item.decided}>
  <div class="row">
    <span class="ic"><Icon name="shield" size={15} /></span>
    <div class="txt">
      <div class="q">{t('permission.askPrefix')}<b>{item.toolName}</b>{t('permission.askSuffix')}</div>
      <div class="sum mono">{item.toolName === 'Bash' ? `$ ${item.input.command}` : toolSummary(item.toolName, item.input)}</div>
      {#if item.toolName === 'Bash' && item.input.description}<div class="reason">{item.input.description}</div>{/if}
      {#if item.reason}<div class="reason">{item.reason}</div>{/if}
    </div>
    {#if !item.decided}
      <button class="btn" disabled={busy} onclick={() => decide(false)}>{t('permission.deny')}</button>
      {#if always}<button class="btn" disabled={busy} title={alwaysHint} onclick={() => decide(true, true)}>{t('permission.alwaysAllow')}</button>{/if}
      <button class="btn primary" disabled={busy} onclick={() => decide(true)}>{t('permission.allow')}</button>
    {:else}
      <span class="badge" class:accent={item.decided === 'allow' || item.decided === 'always'}>
        {item.decided === 'allow' ? t('permission.allowed') : item.decided === 'always' ? t('permission.alwaysAllowed') : item.decided === 'deny' ? t('permission.denied') : t('permission.cancelled')}
      </span>
    {/if}
  </div>
  <button class="toggle" onclick={() => (showInput = !showInput)}>{showInput ? t('permission.hideDetails') : t('permission.showDetails')}</button>
  {#if showInput}<pre class="mono selectable">{JSON.stringify(item.input, null, 2)}</pre>{/if}
</div>

<style>
  .perm {
    border: 1px solid color-mix(in srgb, var(--amber) 50%, var(--border));
    background: color-mix(in srgb, var(--amber) 7%, var(--bg-panel));
    border-radius: var(--radius);
    padding: 10px 12px;
  }
  .perm.done {
    border-color: var(--border);
    background: var(--bg-panel);
    opacity: 0.8;
  }
  .row {
    display: flex;
    align-items: center;
    gap: 10px;
  }
  .ic {
    color: var(--amber);
    display: inline-flex;
  }
  .txt {
    flex: 1;
    min-width: 0;
  }
  .sum {
    color: var(--text-muted);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .reason {
    font-size: 12px;
    color: var(--text-muted);
  }
  .toggle {
    background: none;
    border: none;
    padding: 0;
    margin-top: 6px;
    font-size: 11.5px;
    color: var(--text-faint);
  }
  pre {
    margin: 6px 0 0;
    white-space: pre-wrap;
    max-height: 260px;
    overflow: auto;
  }
</style>
