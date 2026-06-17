<script lang="ts">
  import { onMount } from "svelte";
  import {
    configHasGoogle,
    configSetGoogle,
    gmailStatus,
    gmailConnect,
    gmailDisconnect,
    mailList,
    mailSearch,
    mailGet,
    mailMarkRead,
    type MailSummary,
    type MailDetail,
    type GmailStatus,
  } from "$lib/api";

  let hasConfig = $state(false);
  let clientIdInput = $state("");
  let clientSecretInput = $state("");
  let busy = $state(false);
  let message = $state("");

  // Gmail connection state
  let status = $state<GmailStatus>({ connected: false, email: null });

  // Mail list & search
  let mailList_ = $state<MailSummary[]>([]);
  let searchQuery = $state("");
  let selectedMailId = $state<string | null>(null);
  let selectedMailDetail = $state<MailDetail | null>(null);

  onMount(async () => {
    try {
      hasConfig = await configHasGoogle();
      if (hasConfig) {
        status = await gmailStatus();
        if (status.connected) {
          await loadMailList();
        }
      }
    } catch (e) {
      message = `初始化失败 / Initialization failed: ${e}`;
    }
  });

  async function saveConfig() {
    const id = clientIdInput.trim();
    const secret = clientSecretInput.trim();
    if (!id || !secret) {
      message = "请填入 Client ID 和 Secret / Please fill in Client ID and Secret";
      return;
    }
    busy = true;
    try {
      await configSetGoogle(id, secret);
      clientIdInput = "";
      clientSecretInput = "";
      hasConfig = true;
      message = "已保存，请点击连接 Gmail / Saved, click Connect Gmail";
    } catch (e) {
      message = `保存失败 / Save failed: ${e}`;
    } finally {
      busy = false;
    }
  }

  async function connectGmail() {
    busy = true;
    message = "";
    try {
      status = await gmailConnect();
      if (status.connected) {
        message = `已连接 ${status.email} / Connected ${status.email}`;
        await loadMailList();
      } else {
        message = "连接失败 / Connection failed";
      }
    } catch (e) {
      message = `连接失败 / Connection failed: ${e}`;
    } finally {
      busy = false;
    }
  }

  async function disconnect() {
    if (!confirm("确定断开连接？/ Disconnect?")) return;
    busy = true;
    try {
      await gmailDisconnect();
      status = { connected: false, email: null };
      mailList_ = [];
      selectedMailId = null;
      selectedMailDetail = null;
      message = "已断开连接 / Disconnected";
    } catch (e) {
      message = `断开失败 / Disconnect failed: ${e}`;
    } finally {
      busy = false;
    }
  }

  async function loadMailList() {
    busy = true;
    try {
      mailList_ = await mailList();
      selectedMailId = null;
      selectedMailDetail = null;
    } catch (e) {
      message = `加载失败 / Load failed: ${e}`;
    } finally {
      busy = false;
    }
  }

  async function onSearch(e: Event) {
    const q = (e.target as HTMLInputElement).value.trim();
    if (!q) {
      await loadMailList();
      return;
    }
    busy = true;
    try {
      mailList_ = await mailSearch(q);
      selectedMailId = null;
      selectedMailDetail = null;
    } catch (e) {
      message = `搜索失败 / Search failed: ${e}`;
    } finally {
      busy = false;
    }
  }

  async function selectMail(id: string) {
    selectedMailId = id;
    busy = true;
    try {
      selectedMailDetail = await mailGet(id);
      if (selectedMailDetail.unread) {
        await mailMarkRead(id, true);
        selectedMailDetail.unread = false;
        // Update list
        const idx = mailList_.findIndex((m) => m.id === id);
        if (idx >= 0) {
          mailList_[idx].unread = false;
        }
      }
    } catch (e) {
      message = `加载邮件失败 / Load mail failed: ${e}`;
    } finally {
      busy = false;
    }
  }

  async function toggleReadState(id: string, currentUnread: boolean) {
    try {
      await mailMarkRead(id, currentUnread);
      if (selectedMailDetail) {
        selectedMailDetail.unread = currentUnread;
      }
      const idx = mailList_.findIndex((m) => m.id === id);
      if (idx >= 0) {
        mailList_[idx].unread = currentUnread;
      }
    } catch (e) {
      message = `标记失败 / Mark failed: ${e}`;
    }
  }
</script>

<main class="container">
  <h1>邮件 / Mail</h1>

  {#if !hasConfig}
    <section class="card">
      <h2>配置 Google 客户端 / Configure Google Client</h2>
      <p>申请步骤见 Google Cloud Console 文档。</p>
      <p>See Google Cloud Console documentation for setup steps.</p>
      <div class="form">
        <input
          placeholder="Client ID (from Google Cloud)"
          bind:value={clientIdInput}
          disabled={busy}
        />
        <input
          placeholder="Client Secret (from Google Cloud)"
          bind:value={clientSecretInput}
          disabled={busy}
        />
        <button onclick={saveConfig} disabled={busy}>保存 / Save</button>
      </div>
    </section>
  {:else if !status.connected}
    <section class="card">
      <p>请点击下方按钮连接 Gmail / Click below to connect Gmail</p>
      <button onclick={connectGmail} disabled={busy}>连接 Gmail / Connect</button>
    </section>
  {:else}
    <div class="mail-container">
      <div class="mail-header">
        <span>已连接 / Connected: {status.email}</span>
        <button class="ghost" onclick={disconnect} disabled={busy}>断开 / Disconnect</button>
      </div>

      <div class="search-bar">
        <input
          placeholder="搜索邮件... / Search mail..."
          bind:value={searchQuery}
          onkeydown={(e) => e.key === "Enter" && onSearch(e)}
          disabled={busy}
        />
        <button onclick={() => loadMailList()} disabled={busy}>刷新 / Refresh</button>
      </div>

      {#if message}
        <p class="msg">{message}</p>
      {/if}

      <div class="mail-split">
        <div class="mail-list">
          {#each mailList_ as mail (mail.id)}
            <button
              class="mail-item"
              class:selected={selectedMailId === mail.id}
              class:unread={mail.unread}
              onclick={() => selectMail(mail.id)}
              disabled={busy}
            >
              {#if mail.unread}
                <span class="unread-dot">●</span>
              {/if}
              <div class="mail-item-content">
                <div class="mail-from" class:bold={mail.unread}>{mail.from}</div>
                <div class="mail-subject" class:bold={mail.unread}>{mail.subject}</div>
                <div class="mail-snippet">{mail.snippet}</div>
              </div>
            </button>
          {/each}
          {#if mailList_.length === 0}
            <div class="empty">没有邮件 / No mail</div>
          {/if}
        </div>

        <div class="mail-detail">
          {#if selectedMailDetail}
            <div class="detail-header">
              <div>
                <div class="detail-label">From:</div>
                <div class="detail-value">{selectedMailDetail.from}</div>
              </div>
              <div>
                <div class="detail-label">To:</div>
                <div class="detail-value">{selectedMailDetail.to}</div>
              </div>
              <div>
                <div class="detail-label">Subject:</div>
                <div class="detail-value">{selectedMailDetail.subject}</div>
              </div>
              <div>
                <div class="detail-label">Date:</div>
                <div class="detail-value">{selectedMailDetail.date}</div>
              </div>
              <div class="detail-actions">
                {#if selectedMailDetail}
                  {@const detail = selectedMailDetail}
                  <button
                    class="ghost"
                    onclick={() => toggleReadState(detail.id, !detail.unread)}
                    disabled={busy}
                  >
                    {detail.unread ? "标已读 / Mark Read" : "标未读 / Mark Unread"}
                  </button>
                {/if}
              </div>
            </div>

            <div class="detail-body">
              {#if selectedMailDetail.is_html}
                <!-- svelte-ignore a11y_missing_attribute -->
                <iframe sandbox="" srcdoc={selectedMailDetail.body}></iframe>
              {:else}
                <pre>{selectedMailDetail.body}</pre>
              {/if}
            </div>
          {:else}
            <div class="detail-placeholder">选择邮件查看详情 / Select a mail to view</div>
          {/if}
        </div>
      </div>
    </div>
  {/if}
</main>

<style>
  .container {
    max-width: 100%;
    height: 100%;
    display: flex;
    flex-direction: column;
    padding: 1rem;
  }

  h1 {
    margin-top: 0;
  }

  .card {
    border: 1px solid var(--border);
    border-radius: 10px;
    padding: 1.5rem;
    margin-bottom: 1rem;
  }

  h2 {
    margin-top: 0;
    font-size: 1.05rem;
  }

  .form {
    display: flex;
    flex-direction: column;
    gap: 0.6rem;
  }

  input,
  button {
    border-radius: 8px;
    border: 1px solid var(--border);
    padding: 0.5em 0.8em;
    font-size: 1em;
    color: var(--fg);
    background: var(--surface);
    cursor: pointer;
  }

  input {
    cursor: text;
  }

  button:disabled {
    opacity: 0.6;
    cursor: not-allowed;
  }

  .ghost {
    border-color: transparent;
    background: transparent;
  }

  .msg {
    opacity: 0.85;
    margin: 0.5rem 0;
  }

  .mail-container {
    flex: 1;
    display: flex;
    flex-direction: column;
    gap: 0.8rem;
  }

  .mail-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 0.5rem;
    border-bottom: 1px solid var(--border);
  }

  .search-bar {
    display: flex;
    gap: 0.5rem;
  }

  .search-bar input {
    flex: 1;
  }

  .mail-split {
    flex: 1;
    display: grid;
    grid-template-columns: 300px 1fr;
    gap: 0.8rem;
    min-height: 0;
  }

  .mail-list {
    border: 1px solid var(--border);
    border-radius: 8px;
    overflow-y: auto;
    display: flex;
    flex-direction: column;
  }

  .mail-item {
    flex: none;
    padding: 0.8rem;
    border-bottom: 1px solid var(--border);
    display: flex;
    gap: 0.5rem;
    align-items: flex-start;
    text-align: left;
    background: transparent;
    border: none;
    border-radius: 0;
    border-bottom: 1px solid var(--border);
    cursor: pointer;
    color: var(--fg);
  }

  .mail-item:hover {
    background: rgba(255, 255, 255, 0.05);
  }

  .mail-item.selected {
    background: rgba(100, 150, 255, 0.1);
  }

  .unread-dot {
    color: #64b5ff;
    margin-top: 0.1rem;
  }

  .mail-item-content {
    flex: 1;
    min-width: 0;
  }

  .mail-from,
  .mail-subject {
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .mail-from {
    font-size: 0.95rem;
  }

  .mail-subject {
    font-size: 0.85rem;
    opacity: 0.8;
  }

  .mail-snippet {
    font-size: 0.75rem;
    opacity: 0.6;
    margin-top: 0.2rem;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .bold {
    font-weight: 600;
  }

  .empty {
    padding: 2rem 1rem;
    text-align: center;
    opacity: 0.6;
  }

  .mail-detail {
    border: 1px solid var(--border);
    border-radius: 8px;
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }

  .detail-placeholder {
    flex: 1;
    display: flex;
    align-items: center;
    justify-content: center;
    opacity: 0.5;
    color: var(--fg);
  }

  .detail-header {
    padding: 1rem;
    border-bottom: 1px solid var(--border);
    flex: none;
  }

  .detail-label {
    font-size: 0.8rem;
    opacity: 0.7;
    margin-bottom: 0.2rem;
  }

  .detail-value {
    font-size: 0.95rem;
    margin-bottom: 0.6rem;
    word-break: break-word;
  }

  .detail-actions {
    margin-top: 0.5rem;
  }

  .detail-body {
    flex: 1;
    overflow-y: auto;
    padding: 1rem;
    min-height: 0;
  }

  .detail-body iframe {
    width: 100%;
    height: 100%;
    border: none;
    background: var(--surface);
  }

  .detail-body pre {
    margin: 0;
    white-space: pre-wrap;
    word-break: break-word;
    font-size: 0.9rem;
    font-family: monospace;
  }
</style>
