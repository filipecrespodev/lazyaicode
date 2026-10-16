<script lang="ts">
  import { app } from '../stores/app.svelte'
  import { api, errText, type BranchInfo, type Changes, type Thread } from '../api'
  import Icon from './Icon.svelte'
  import DiffFile from './DiffFile.svelte'
  import { t } from '../i18n/index.svelte'

  // Revisão do que o agente mudou nesta thread: desde a base, desde o último
  // commit ou só o último turno, com marcação de arquivos vistos.
  let { thread, visible }: { thread: Thread; visible: boolean } = $props()

  let base = $state<'upstream' | 'head' | 'turn'>('upstream')
  let changes = $state<Changes | null>(null)
  let loading = $state(false)
  let error = $state('')
  let collapsed = $state<Record<string, boolean>>({})
  let info = $state<BranchInfo | null>(null)

  const files = $derived(changes?.files ?? [])
  const seenCount = $derived(files.filter((f) => f.seen).length)
  const totals = $derived(files.reduce((a, f) => [a[0] + f.additions, a[1] + f.deletions], [0, 0]))

  async function load() {
    loading = true
    error = ''
    try {
      const [c, i] = await Promise.all([api.getChanges(thread.id, base), api.branchInfo(thread.id).catch(() => null)])
      changes = c
      info = i
      for (const f of c.files) if (f.seen && collapsed[f.path] === undefined) collapsed[f.path] = true
    } catch (e) {
      error = errText(e)
      changes = null
    } finally {
      loading = false
    }
  }

  // recarrega ao trocar de thread/base, ao fim de cada turno e periodicamente enquanto o agente trabalha
  $effect(() => {
    void thread.id
    void base
    void app.changesTick
    if (visible) load()
  })
  $effect(() => {
    if (!visible || thread.status !== 'running') return
    const timer = setInterval(load, 4000)
    return () => clearInterval(timer)
  })
  $effect(() => {
    void thread.id
    collapsed = {}
  })

  async function setSeen(path: string, fp: string, seen: boolean) {
    const f = files.find((x) => x.path === path)
    if (f) f.seen = seen
    collapsed[path] = seen
    await api.setSeen(thread.id, [[path, seen ? fp : null]])
  }

  async function allSeen(seen: boolean) {
    for (const f of files) {
      f.seen = seen
      collapsed[f.path] = seen
    }
    await api.setSeen(
      thread.id,
      files.map((f) => [f.path, seen ? f.fingerprint : null]),
    )
  }

  function setAll(c: boolean) {
    for (const f of files) collapsed[f.path] = c
  }

</script>

<div class="review">
    <div class="toolbar">
      <select class="select" bind:value={base} title={t('agentReview.compareWith')}>
        <option value="upstream">{t('agentReview.sinceBase', { base: thread.base_ref ?? t('agentReview.theBase') })}</option>
        <option value="head">{t('agentReview.sinceLastCommit')}</option>
        <option value="turn">{t('agentReview.lastTurn')}</option>
      </select>
      {#if files.length}
        <span class="progress" title={t('agentReview.filesSeen')}>{t('agentReview.seenCount', { seen: seenCount, total: files.length })}</span>
        <span class="tot mono"><span class="plus">+{totals[0]}</span> <span class="minus">−{totals[1]}</span></span>
      {/if}
      <span class="sp"></span>
      <button class="btn ghost icon" title={t('agentReview.collapseAll')} onclick={() => setAll(true)}><Icon name="down" size={13} /></button>
      <button class="btn ghost icon" title={t('agentReview.markAllSeen')} onclick={() => allSeen(seenCount < files.length)}><Icon name="eye" size={13} /></button>
      <button class="btn ghost icon" title={t('agentReview.refresh')} onclick={load} disabled={loading}><Icon name="refresh" size={13} /></button>
    </div>
    {#if info}
      <div class="branchbar mono">
        <Icon name="branch" size={12} /> {info.branch}
        <span class="faint">· {t('agentReview.aheadBehind', { ahead: info.ahead, behind: info.behind, base: info.base_ref })}</span>
        {#if info.dirty}<span class="badge">{t('agentReview.uncommitted')}</span>{/if}
      </div>
    {/if}
    <div class="files">
      {#if error}
        <div class="empty err">{error}</div>
      {:else if !changes && loading}
        <div class="empty">{t('agentReview.computing')}</div>
      {:else if !files.length}
        <div class="empty">{t(base === 'turn' ? 'agentReview.noChangesTurn' : 'agentReview.noChanges')}</div>
      {/if}
      {#each files as f (f.path)}
        <DiffFile
          file={f}
          collapsed={!!collapsed[f.path]}
          onToggle={() => (collapsed[f.path] = !collapsed[f.path])}
          onSeen={(s) => setSeen(f.path, f.fingerprint, s)}
        />
      {/each}
    </div>
</div>

<style>
  .sp {
    flex: 1;
  }
  .review {
    height: 100%;
    display: flex;
    flex-direction: column;
    min-width: 0;
    flex: 1;
    min-height: 0;
  }
  .toolbar {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 6px 8px;
    border-bottom: 1px solid var(--border);
  }
  .progress {
    font-size: 11.5px;
    color: var(--text-muted);
  }
  .tot {
    font-size: 11px;
  }
  .plus {
    color: var(--add-fg);
  }
  .minus {
    color: var(--del-fg);
  }
  .branchbar {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 5px 12px;
    font-size: 11px;
    color: var(--text-muted);
    border-bottom: 1px solid var(--border);
  }
  .faint {
    color: var(--text-faint);
  }
  .files {
    flex: 1;
    overflow-y: auto;
    padding: 10px;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .empty {
    color: var(--text-muted);
    text-align: center;
    padding: 40px 10px;
  }
  .empty.err {
    color: var(--red);
    user-select: text;
  }
</style>
