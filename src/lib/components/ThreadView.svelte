<script lang="ts">
  import { tick } from 'svelte'
  import { app } from '../stores/app.svelte'
  import { api, errText, PROVIDERS, type Thread } from '../api'
  import { renderMarkdown } from '../markdown'
  import { emptyState } from '../transcript'
  import Icon from './Icon.svelte'
  import ToolCard from './ToolCard.svelte'
  import PermissionCard from './PermissionCard.svelte'
  import Avatar from './Avatar.svelte'
  import SessionFooter from './SessionFooter.svelte'
  import MainTabs from './MainTabs.svelte'
  import GitPanel from './GitPanel.svelte'
  import AgentReview from './AgentReview.svelte'
  import BranchMenu from './BranchMenu.svelte'
  import { rankCommands, slashQuery } from '../slash'
  import { MAX_IMAGES, clipboardImage, dataUrl, dragHasFiles, filesFromPaths, imageFiles, imagePaths, mayHaveNativeImage, readImages, type Attachment } from '../images'
  import type { CapItem } from '../api'
  import { intlLocale, t } from '../i18n/index.svelte'
  import { keyHint } from '../shortcuts/store.svelte'

  let { thread }: { thread: Thread } = $props()

  let draft = $state('')
  let drafts: Record<string, string> = {}
  /** imagens anexadas à próxima mensagem (Ctrl+V ou arrastar), por thread como o rascunho */
  let attachments = $state<Attachment[]>([])
  let draftImages: Record<string, Attachment[]> = {}
  /** contador de dragenter/dragleave: os filhos disparam os dois ao passar por cima */
  let dragDepth = $state(0)
  /** imagem aberta em tamanho real */
  let preview = $state<string | null>(null)
  let scroller: HTMLDivElement | undefined = $state()
  let textarea: HTMLTextAreaElement | undefined = $state()
  let stick = true
  let landOpen = $state(false)
  let editingTitle = $state(false)
  let titleValue = $state('')
  let thinkingOpen = $state<Record<string, boolean>>({})
  let width = $state(1000)
  /** abas Git de tela inteira (Mudanças tem o próprio painel, com a revisão) */
  const GIT_VIEWS = ['tree', 'reflog', 'stats'] as const
  /** aba Mudanças: staging do git ou revisão do que o agente mudou */
  let historyView = $state<'graph' | 'review'>('graph')

  // leitura pura: criar o estado aqui seria mutação dentro de um $derived
  const EMPTY = emptyState()
  const tr = $derived(app.transcripts[thread.id] ?? EMPTY)
  const busy = $derived(thread.status === 'running' || thread.status === 'awaiting')
  const project = $derived(app.projects.find((p) => p.id === thread.project_id))
  const projectPath = $derived(project?.path ?? null)

  // Como numa conversa: cada mensagem do usuário é uma linha com o avatar
  // dele; tudo que o agente faz até a próxima mensagem fica sob um avatar só.
  const rows = $derived(
    tr.items.map((item, i) => ({ item, head: item.kind !== 'user' && (i === 0 || tr.items[i - 1].kind === 'user') })),
  )
  const userName = $derived(app.profile?.name ?? t('thread.you'))

  // branch atual do checkout da thread; relida ao trocar de thread e quando o agente para
  let currentBranch = $state<string | null>(null)
  let branchFor = ''
  $effect(() => {
    const id = thread.id
    if (id !== branchFor) {
      branchFor = id
      currentBranch = null
    }
    if (busy) return
    api
      .branchInfo(id)
      .then((b) => { if (thread.id === id) currentBranch = b.branch || null })
      .catch(() => { if (thread.id === id) currentBranch = null })
  })
  const branchName = $derived(currentBranch ?? thread.branch)
  /** caminho com a pasta pessoal abreviada como `~` */
  const shortPath = (p: string) => p.replace(/^\/home\/[^/]+|^\/Users\/[^/]+|^[A-Za-z]:\\Users\\[^\\]+/, '~')

  // ---- menu "/" (skills e comandos), como no Delta ----
  let caret = $state(0)
  let slashIdx = $state(0)
  let slashDismissed = $state(false)
  const slashQ = $derived(slashDismissed ? null : slashQuery(draft, caret))
  const caps = $derived(app.capabilities[app.capKey(projectPath)])
  // Codex/Gemini anunciam os próprios comandos (available_commands_update)
  const acpCommands = $derived<CapItem[] | null>(
    thread.provider === 'claude'
      ? null
      : (app.acpSession[thread.id]?.commands ?? []).map((c) => ({
          kind: 'command', name: c.name, description: c.description ?? '', scope: 'other', plugin: null, path: '',
          model: null, tools: null, error: null, loaded: true, builtin: false, argument_hint: c.hint ?? null,
        })),
  )
  const slashSource = $derived(acpCommands ?? caps?.commands ?? null)
  const slashItems = $derived<CapItem[]>(slashQ === null || !slashSource ? [] : rankCommands(slashSource, slashQ))
  const slashSel = $derived(slashItems[Math.min(slashIdx, slashItems.length - 1)])
  const SCOPE_KEY: Record<string, string> = { project: 'thread.scopeProject', global: 'thread.scopeGlobal', plugin: 'thread.scopePlugin', other: 'thread.scopeOther', builtin: 'thread.scopeBuiltin' }

  $effect(() => {
    if (slashQ !== null) {
      if (thread.provider === 'claude') app.loadCapabilities(projectPath)
      // o menu abre abaixo do campo: garante que ele fique à vista
      tick().then(scrollBottom)
    }
  })
  $effect(() => {
    void slashQ
    slashIdx = 0
  })

  function trackCaret() {
    caret = textarea?.selectionStart ?? draft.length
  }

  async function pickSlash(i: CapItem) {
    const rest = draft.slice(caret).replace(/^\S*/, '')
    draft = `/${i.name} ${rest.trimStart()}`
    const pos = i.name.length + 2
    await tick()
    textarea?.setSelectionRange(pos, pos)
    caret = pos
    slashDismissed = true
    textarea?.focus()
    autosize()
  }

  // rascunho por thread
  let lastId = ''
  $effect(() => {
    const id = thread.id
    if (id !== lastId) {
      if (lastId) {
        drafts[lastId] = draft
        draftImages[lastId] = attachments
      }
      draft = drafts[id] ?? ''
      attachments = draftImages[id] ?? []
      dragDepth = 0
      lastId = id
      stick = true
      tick().then(() => {
        scrollBottom()
        textarea?.focus()
      })
    }
  })

  $effect(() => {
    // acompanha o fim enquanto o usuário não rolou para cima
    void tr.items.length
    const last = tr.items[tr.items.length - 1]
    if (last && 'text' in last) void last.text
    if (stick) tick().then(scrollBottom)
  })

  function scrollBottom() {
    if (scroller) scroller.scrollTop = scroller.scrollHeight
  }

  function onScroll() {
    if (!scroller) return
    stick = scroller.scrollHeight - scroller.scrollTop - scroller.clientHeight < 60
  }

  function autosize() {
    if (!textarea) return
    textarea.style.height = 'auto'
    textarea.style.height = Math.min(textarea.scrollHeight, 320) + 'px'
  }

  const canSend = $derived((!!draft.trim() || attachments.length > 0) && !thread.archived)

  async function send() {
    const text = draft.trim()
    if ((!text && !attachments.length) || busy || thread.archived) return
    const images = attachments
    draft = ''
    attachments = []
    stick = true
    await tick()
    autosize()
    await app.send(text, images)
  }

  async function attach(files: File[], errors: string[] = []) {
    if (thread.archived) return
    const read = await readImages(files, MAX_IMAGES - attachments.length)
    attachments = [...attachments, ...read.added]
    for (const msg of [...errors, ...read.errors]) app.toast(msg, 'error')
    textarea?.focus()
  }

  /** Arquivos de imagem do evento; no Linux eles podem vir só como caminhos. */
  async function attachFrom(files: File[], paths: string[]) {
    const fromDisk = files.length ? { files: [], errors: [] } : await filesFromPaths(paths)
    await attach([...files, ...fromDisk.files], fromDisk.errors)
  }

  function onPaste(e: ClipboardEvent) {
    const files = imageFiles(e.clipboardData)
    const paths = files.length ? [] : imagePaths(e.clipboardData)
    if (files.length || paths.length) {
      // só a imagem: sem isso alguns sistemas colam também o nome do arquivo
      e.preventDefault()
      attachFrom(files, paths)
      return
    }
    // o WebKitGTK não expõe a imagem de um print no evento: lê do sistema
    if (!mayHaveNativeImage(e.clipboardData)) return
    e.preventDefault()
    clipboardImage().then((f) => f && attach([f]))
  }

  function onDragEnter(e: DragEvent) {
    if (!dragHasFiles(e.dataTransfer)) return
    e.preventDefault()
    dragDepth++
  }
  function onDragOver(e: DragEvent) {
    if (!dragHasFiles(e.dataTransfer)) return
    e.preventDefault()
    if (e.dataTransfer) e.dataTransfer.dropEffect = thread.archived ? 'none' : 'copy'
  }
  function onDragLeave(e: DragEvent) {
    if (!dragHasFiles(e.dataTransfer)) return
    dragDepth = Math.max(0, dragDepth - 1)
  }
  function onDrop(e: DragEvent) {
    if (!dragHasFiles(e.dataTransfer)) return
    e.preventDefault()
    dragDepth = 0
    const files = imageFiles(e.dataTransfer)
    const paths = files.length ? [] : imagePaths(e.dataTransfer)
    if (files.length || paths.length) attachFrom(files, paths)
    else app.toast(t('thread.onlyImages'), 'error')
  }

  function onKey(e: KeyboardEvent) {
    if (slashQ !== null && slashItems.length) {
      if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
        e.preventDefault()
        const n = slashItems.length
        slashIdx = (slashIdx + (e.key === 'ArrowDown' ? 1 : -1) + n) % n
        return
      }
      if ((e.key === 'Enter' && !e.shiftKey) || e.key === 'Tab') {
        e.preventDefault()
        if (slashSel) pickSlash(slashSel)
        return
      }
      if (e.key === 'Escape') {
        e.preventDefault()
        slashDismissed = true
        return
      }
    }
    const mod = app.settings?.send_with_modifier
    if (e.key === 'Enter' && !e.shiftKey && (mod ? e.ctrlKey || e.metaKey : !e.ctrlKey)) {
      e.preventDefault()
      send()
    } else if (e.key === 'Escape' && busy) {
      e.preventDefault()
      app.interrupt()
    }
  }

  async function saveTitle() {
    editingTitle = false
    const v = titleValue.trim()
    if (v && v !== thread.title) app.upsert(await api.updateThreadMeta(thread.id, { title: v }))
  }

  async function land(mode: 'merge' | 'apply' | 'pr') {
    landOpen = false
    if (mode === 'pr') {
      await app.send(t('thread.prPrompt'))
      return
    }
    try {
      const msg = await api.landChanges(thread.id, mode)
      app.toast(msg, 'ok')
    } catch (e) {
      app.toast(errText(e), 'error')
    }
  }

  const fmtCost = (usd: number) =>
    new Intl.NumberFormat(intlLocale(), { style: 'currency', currency: 'USD', minimumFractionDigits: 4, maximumFractionDigits: 4 }).format(usd)

  function fmtDuration(ms: number) {
    return ms < 60000 ? `${(ms / 1000).toFixed(1)}s` : `${Math.floor(ms / 60000)}m${Math.round((ms % 60000) / 1000)}s`
  }
</script>

<svelte:window onclick={() => (landOpen = false)} onkeydown={(e) => preview && e.key === 'Escape' && (preview = null)} />

<section class="thread" bind:clientWidth={width} class:narrow={width < 720} class:collapsed={app.tab === 'terminal'}>
  <header class="head">
    <div class="crumbs">
      {#if editingTitle}
        <!-- svelte-ignore a11y_autofocus -->
        <input class="title-input" bind:value={titleValue} autofocus onblur={saveTitle}
          onkeydown={(e) => { if (e.key === 'Enter') saveTitle(); if (e.key === 'Escape') editingTitle = false }} />
      {:else}
        <button class="title" ondblclick={() => { editingTitle = true; titleValue = thread.title }} title={t('thread.renameHint')}>{thread.title}</button>
      {/if}
      <div class="meta">
        <span class="pname">{project?.name}</span>
        <BranchMenu path={thread.cwd} branch={branchName} disabled={busy} onChanged={(b) => (currentBranch = b)} />
        {#if thread.provider !== 'claude'}<span class="badge prov-{thread.provider}">{PROVIDERS.find((p) => p.id === thread.provider)?.label}</span>{/if}
        {#if thread.isolated}
          <span class="badge" title={thread.cwd}><Icon name="layers" size={11} /> {t('thread.isolatedWorkspace')}</span>
        {:else}
          <span class="badge mono path" title={t('thread.localCheckout', { path: thread.cwd })}><Icon name="folder" size={11} /> <span class="pt">{shortPath(thread.cwd)}</span></span>
        {/if}
      </div>
    </div>
    <div class="tools">
      <button class="btn ghost icon" class:on={thread.bookmarked} title={t('thread.favorite', { key: keyHint('toggleBookmark') })}
        onclick={async () => app.upsert(await api.updateThreadMeta(thread.id, { bookmarked: !thread.bookmarked }))}>
        <Icon name="bookmark" />
      </button>
      {#if thread.isolated}
        <div class="land">
          <button class="btn" onclick={(e) => { e.stopPropagation(); landOpen = !landOpen }} title={t('thread.landTitle')}>
            <Icon name="merge" /> {t('thread.land')} <Icon name="down" size={12} />
          </button>
          {#if landOpen}
            <div class="menu">
              <button onclick={() => land('apply')}>
                <b>{t('thread.applyTitle')}</b><span>{t('thread.applyDesc')}</span>
              </button>
              <button onclick={() => land('merge')}>
                <b>{t('thread.mergeTitle')}</b><span>{t('thread.mergeDescPrefix')}<code>{thread.branch}</code>{t('thread.mergeDescSuffix')}</span>
              </button>
              <button onclick={() => land('pr')}>
                <b>{t('thread.prTitle')}</b><span>{t('thread.prDesc')}</span>
              </button>
            </div>
          {/if}
        </div>
      {/if}
      <button class="btn ghost icon" title={t('thread.archiveThread')} onclick={() => app.archive(thread.id, !thread.archived)}>
        <Icon name="archive" />
      </button>
    </div>
  </header>

  <MainTabs repoPath={thread.cwd} git={app.isGit} />

  {#if app.isGit}
    <!-- Histórico: grafo + mudanças locais (staging/commit), ou a revisão do que o agente mudou -->
    <div class="pane" class:hidden={app.tab !== 'history'}>
      <div class="subbar">
        <div class="seg">
          <button class:on={historyView === 'graph'} onclick={() => (historyView = 'graph')}>{t('mainTabs.history')}</button>
          <button class:on={historyView === 'review'} onclick={() => (historyView = 'review')}>{t('thread.agentReview')}</button>
        </div>
      </div>
      <div class="pane" class:hidden={historyView !== 'graph'}>
        <GitPanel path={thread.cwd} mode="history" active={app.tab === 'history' && historyView === 'graph'} />
      </div>
      {#if historyView === 'review'}
        <AgentReview {thread} visible={app.tab === 'history'} />
      {/if}
    </div>
    {#each GIT_VIEWS as m (m)}
      <div class="pane" class:hidden={app.tab !== m}>
        <GitPanel path={thread.cwd} mode={m} active={app.tab === m} />
      </div>
    {/each}
  {/if}

  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div class="chat" class:hidden={app.tab !== 'chat'} class:dropping={dragDepth > 0}
    ondragenter={onDragEnter} ondragover={onDragOver} ondragleave={onDragLeave} ondrop={onDrop}>
  {#if dragDepth > 0}
    <div class="drop-hint">{thread.archived ? t('thread.threadArchived') : t('thread.dropToAttach')}</div>
  {/if}
  <div class="scroll" bind:this={scroller} onscroll={onScroll}>
    <div class="col" class:slashing={slashQ !== null}>
      {#if !tr.items.length}
        <div class="hello">
          {#if thread.isolated}
            {t('thread.helloIsolatedA')}<code>{thread.branch}</code>{t('thread.helloIsolatedB')}<code>{thread.base_ref}</code>{t('thread.helloIsolatedC')}
          {:else}
            {t('thread.helloDirectPrefix')}<code>{thread.cwd}</code>{t('thread.helloDirectSuffix')}
          {/if}
        </div>
      {/if}
      {#each rows as { item, head } (item.key)}
        {#if item.kind === 'user'}
          <div class="row user">
            <Avatar kind="user" src={app.profile?.avatar} name={userName} />
            <div class="body selectable">
              {#if item.images?.length}
                <div class="shots">
                  {#each item.images as src, n (n)}
                    <button class="shot" title={t('thread.viewImage')} onclick={() => (preview = src)}><img {src} alt={t('thread.attachedImage')} /></button>
                  {/each}
                </div>
              {/if}
              {item.text}
            </div>
          </div>
        {:else}
          <div class="row agent" class:lead={head}>
            {#if head}<Avatar kind="agent" />{:else}<span class="gutter"></span>{/if}
            <div class="body">
              {#if item.kind === 'text'}
                <div class="md">{@html renderMarkdown(item.text)}</div>
              {:else if item.kind === 'thinking'}
                <button class="thinking" onclick={() => (thinkingOpen[item.key] = !thinkingOpen[item.key])}>
                  <Icon name="brain" size={13} /> {item.final ? t('thread.reasoning') : t('thread.thinking')}
                </button>
                {#if thinkingOpen[item.key] && item.text}<div class="thinking-body selectable">{item.text}</div>{/if}
              {:else if item.kind === 'tool'}
                <ToolCard {item} />
              {:else if item.kind === 'permission'}
                <PermissionCard {item} threadId={thread.id} />
              {:else if item.kind === 'result'}
                <div class="result" class:bad={item.isError}>
                  {#if item.isError}<Icon name="alert" size={12} /> {item.text || t('thread.turnFailed')} ·{/if}
                  {fmtDuration(item.durationMs)}{#if item.costUsd} · {fmtCost(item.costUsd)}{/if}
                </div>
              {:else if item.kind === 'error'}
                <div class="error selectable"><Icon name="alert" size={13} /> <pre>{item.text}</pre></div>
              {:else if item.kind === 'notice'}
                <div class="notice selectable"><Icon name="alert" size={12} /> {item.text}</div>
              {/if}
            </div>
          </div>
        {/if}
      {/each}
      {#if thread.status === 'running'}
        <div class="row agent">
          <span class="gutter"></span>
          <div class="working"><span class="spinner"></span> {t('thread.working')} <span class="kbd">Esc</span> {t('thread.toInterrupt')}</div>
        </div>
      {/if}

      <!-- o campo de digitar é a próxima mensagem da conversa -->
      <div class="row user composer" class:busy>
        <Avatar kind="user" src={app.profile?.avatar} name={userName} />
        <div class="body">
          {#if attachments.length}
            <div class="shots pending">
              {#each attachments as a (a.id)}
                <div class="shot" title={a.name}>
                  <button class="open" onclick={() => (preview = dataUrl(a))}><img src={dataUrl(a)} alt={a.name} /></button>
                  <button class="rm" title={t('thread.remove')} onclick={() => (attachments = attachments.filter((x) => x.id !== a.id))}>
                    <Icon name="x" size={10} />
                  </button>
                </div>
              {/each}
            </div>
          {/if}
          <textarea
            bind:this={textarea}
            bind:value={draft}
            rows="1"
            placeholder={thread.archived ? t('thread.threadArchived') : busy ? t('thread.agentWorking') : tr.items.length ? t('thread.reply') : t('thread.describeTask')}
            disabled={thread.archived}
            oninput={() => { autosize(); trackCaret(); if (!draft.startsWith('/')) slashDismissed = false }}
            onkeydown={onKey}
            onpaste={onPaste}
            onkeyup={trackCaret}
            onclick={trackCaret}
          ></textarea>
          {#if slashQ !== null}
            <div class="slash">
              {#if !slashSource || (acpCommands && !acpCommands.length)}
                <div class="slash-empty">
                  {acpCommands ? t('thread.acpCommandsLater') : (app.capabilitiesError[app.capKey(projectPath)] ?? t('thread.loadingSkills'))}
                </div>
              {:else if !slashItems.length}
                <div class="slash-empty">{t('thread.noSlashMatch', { q: slashQ })}</div>
              {:else}
                <div class="slash-title">{t('thread.commandsAndSkills')}</div>
                <div class="slash-list">
                  {#each slashItems as i, n (i.scope + i.name)}
                    <button class:on={i === slashSel} onmouseenter={() => (slashIdx = n)} onmousedown={(e) => { e.preventDefault(); pickSlash(i) }}>
                      <span class="nm">{i.name}</span>
                      <span class="ds">{i.description}</span>
                      <span class="sc">{SCOPE_KEY[i.scope] ? t(SCOPE_KEY[i.scope]) : i.scope}</span>
                    </button>
                  {/each}
                </div>
              {/if}
            </div>
          {/if}
        </div>
      </div>
    </div>
  </div>

  <SessionFooter {thread} {projectPath} {busy} {canSend} onSend={send} onStop={() => app.interrupt()} />
  </div>
</section>

{#if preview}
  <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
  <div class="lightbox" onclick={() => (preview = null)}>
    <img src={preview} alt={t('thread.attachedImage')} />
  </div>
{/if}

<style>
  .thread {
    display: flex;
    flex-direction: column;
    flex: 1;
    min-height: 0;
    min-width: 0;
  }
  /* aba Terminal: só cabeçalho e abas; o terminal (TerminalHost) vem logo abaixo */
  .thread.collapsed {
    flex: none;
  }
  .chat,
  .pane {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
  }
  .chat.hidden,
  .pane.hidden {
    display: none;
  }
  .subbar {
    display: flex;
    padding: 6px 12px;
    border-bottom: 1px solid var(--border);
  }
  .seg {
    display: inline-flex;
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
    overflow: hidden;
  }
  .seg button {
    background: none;
    border: none;
    padding: 3px 10px;
    font-size: 12px;
    color: var(--text-muted);
  }
  .seg button.on {
    background: var(--bg-active);
    color: var(--text);
  }
  .head {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 8px 12px 8px 18px;
    border-bottom: 1px solid var(--border);
    min-height: 54px;
  }
  .crumbs {
    flex: 1;
    min-width: 0;
  }
  .title {
    background: none;
    border: none;
    padding: 0;
    font-weight: 600;
    font-size: 14px;
    max-width: 100%;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    display: block;
    text-align: left;
  }
  .title-input {
    font-weight: 600;
    font-size: 14px;
    background: var(--bg-input);
    border: 1px solid var(--border-strong);
    border-radius: 4px;
    outline: none;
    width: 60%;
  }
  .meta {
    display: flex;
    white-space: nowrap;
    overflow: hidden;
    align-items: center;
    gap: 8px;
    font-size: 12px;
    color: var(--text-muted);
    margin-top: 2px;
  }
  .pname {
    flex: none;
  }
  .tools {
    flex: none;
    display: flex;
    align-items: center;
    gap: 4px;
  }
  .path {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    min-width: 0;
    font-size: 11px;
  }
  .meta .path {
    flex: 0 1 auto;
  }
  .path .pt {
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .meta :global(.badge) {
    flex: none;
  }
  .tools .on {
    background: var(--bg-active);
    color: var(--text);
  }
  .land {
    position: relative;
  }
  .menu {
    position: absolute;
    right: 0;
    top: 34px;
    z-index: 30;
    width: 320px;
    background: var(--bg-elev);
    border: 1px solid var(--border-strong);
    border-radius: var(--radius);
    padding: 4px;
    box-shadow: 0 12px 30px rgba(0, 0, 0, 0.35);
  }
  .menu button {
    display: flex;
    flex-direction: column;
    gap: 2px;
    width: 100%;
    text-align: left;
    background: none;
    border: none;
    border-radius: 5px;
    padding: 8px 10px;
  }
  .menu button:hover {
    background: var(--bg-hover);
  }
  .menu span {
    font-size: 12px;
    color: var(--text-muted);
  }
  .scroll {
    flex: 1;
    overflow-y: auto;
  }
  .col {
    max-width: var(--content-w, 820px);
    margin: 0 auto;
    padding: 20px 24px 40px;
    display: flex;
    flex-direction: column;
  }
  .col.slashing {
    padding-bottom: 340px;
  }
  .hello {
    color: var(--text-faint);
    font-size: 12.5px;
    padding: 4px 0 14px 40px;
  }
  .hello code {
    font-size: 11.5px;
    background: var(--bg-panel);
    padding: 1px 5px;
    border-radius: 4px;
  }
  .row {
    display: flex;
    gap: 12px;
    align-items: flex-start;
  }
  .row.user {
    margin-top: 18px;
  }
  .row.user:first-child,
  .hello + .row.user {
    margin-top: 0;
  }
  .row.agent {
    margin-top: 8px;
  }
  .row.agent.lead {
    margin-top: 16px;
  }
  .gutter {
    width: 28px;
    flex: none;
  }
  .body {
    flex: 1;
    min-width: 0;
    padding-top: 4px;
  }
  .row.user > .body {
    white-space: pre-wrap;
    font-size: 14px;
    line-height: 1.55;
    overflow-wrap: anywhere;
  }
  .thinking {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    background: none;
    border: none;
    color: var(--text-faint);
    font-size: 12px;
    padding: 0;
  }
  .thinking-body {
    font-size: 12.5px;
    color: var(--text-muted);
    white-space: pre-wrap;
    border-left: 2px solid var(--border-strong);
    padding-left: 10px;
    margin-top: 4px;
  }
  .result {
    font-size: 11px;
    color: var(--text-faint);
    display: inline-flex;
    gap: 4px;
    align-items: center;
  }
  .result.bad {
    color: var(--red);
  }
  .error {
    display: flex;
    gap: 8px;
    color: var(--red);
    background: color-mix(in srgb, var(--red) 8%, transparent);
    border: 1px solid color-mix(in srgb, var(--red) 35%, transparent);
    border-radius: var(--radius);
    padding: 8px 10px;
  }
  .error pre {
    margin: 0;
    white-space: pre-wrap;
    font-size: 12px;
  }
  .notice {
    display: flex;
    gap: 6px;
    align-items: center;
    font-size: 12px;
    color: var(--amber);
  }
  .working {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 12px;
    color: var(--text-muted);
    padding-top: 4px;
  }
  .composer {
    margin-top: 22px;
  }
  .chat {
    position: relative;
  }
  .drop-hint {
    position: absolute;
    inset: 8px;
    z-index: 20;
    display: flex;
    align-items: center;
    justify-content: center;
    border: 2px dashed var(--accent);
    border-radius: var(--radius);
    background: color-mix(in srgb, var(--bg) 82%, transparent);
    color: var(--accent);
    font-size: 14px;
    pointer-events: none;
  }
  .shots {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
    margin-bottom: 8px;
  }
  .shot {
    position: relative;
    padding: 0;
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
    background: var(--bg-elev);
    overflow: hidden;
    cursor: zoom-in;
  }
  .shot img {
    display: block;
    max-width: 220px;
    max-height: 160px;
    object-fit: contain;
  }
  .shots.pending .shot img {
    width: 64px;
    height: 64px;
    object-fit: cover;
  }
  .shot .open {
    display: block;
    padding: 0;
    border: none;
    background: none;
    cursor: zoom-in;
  }
  .shot .rm {
    position: absolute;
    top: 3px;
    right: 3px;
    width: 18px;
    height: 18px;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    padding: 0;
    border: none;
    border-radius: 50%;
    background: rgba(0, 0, 0, 0.65);
    color: #fff;
    cursor: pointer;
  }
  .lightbox {
    position: fixed;
    inset: 0;
    z-index: 200;
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 32px;
    background: rgba(0, 0, 0, 0.75);
    cursor: zoom-out;
  }
  .lightbox img {
    max-width: 100%;
    max-height: 100%;
    object-fit: contain;
    border-radius: var(--radius-sm);
  }
  .composer .body {
    position: relative;
    padding-top: 3px;
  }
  textarea {
    width: 100%;
    border: none;
    outline: none;
    background: transparent;
    resize: none;
    font-size: 14px;
    line-height: 1.55;
    min-height: 24px;
    padding: 0;
    user-select: text;
  }
  textarea::placeholder {
    color: var(--text-faint);
  }
  .slash {
    position: absolute;
    left: -6px;
    top: calc(100% + 4px);
    width: min(560px, 100%);
    max-height: 320px;
    display: flex;
    flex-direction: column;
    background: var(--bg-elev);
    border: 1px solid var(--border-strong);
    border-radius: 10px;
    box-shadow: 0 12px 30px rgba(0, 0, 0, 0.35);
    overflow: hidden;
    z-index: 20;
  }
  .slash-title {
    font-size: 12px;
    color: var(--text-muted);
    padding: 8px 10px 4px;
  }
  .slash-list {
    overflow-y: auto;
    padding: 2px 4px 4px;
  }
  .slash-list button {
    display: flex;
    align-items: baseline;
    gap: 8px;
    width: 100%;
    padding: 5px 8px;
    border: none;
    border-radius: 6px;
    background: none;
    text-align: left;
    color: var(--text-muted);
  }
  .slash-list button.on {
    background: var(--bg-active);
    color: var(--text);
  }
  .nm {
    color: var(--text);
    font-size: 13.5px;
    white-space: nowrap;
  }
  .ds {
    flex: 1;
    min-width: 0;
    font-size: 12px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .sc {
    font-size: 10.5px;
    color: var(--text-faint);
  }
  .slash-empty {
    padding: 12px;
    color: var(--text-muted);
  }
</style>
