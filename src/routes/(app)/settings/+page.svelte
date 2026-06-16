<script lang="ts">
  import { onMount } from "svelte";
  import { save, open } from "@tauri-apps/plugin-dialog";
  import { autostartGet, autostartSet, dbExport, dbImport } from "$lib/api";

  let autostart = $state(false);
  let busy = $state(false);
  let message = $state("");

  onMount(async () => {
    try {
      autostart = await autostartGet();
    } catch (e) {
      message = `读取自启状态失败 / Failed to read autostart: ${e}`;
    }
  });

  async function toggleAutostart() {
    busy = true;
    try {
      const next = !autostart;
      await autostartSet(next);
      autostart = next;
      message = next ? "已开启开机自启 / Autostart on" : "已关闭开机自启 / Autostart off";
    } catch (e) {
      message = `设置失败 / Failed: ${e}`;
    } finally {
      busy = false;
    }
  }

  function dateStamp(): string {
    const d = new Date();
    const p = (n: number) => String(n).padStart(2, "0");
    return `${d.getFullYear()}${p(d.getMonth() + 1)}${p(d.getDate())}`;
  }

  async function exportBackup() {
    busy = true;
    message = "";
    try {
      const dest = await save({
        defaultPath: `deskhub-backup-${dateStamp()}.db`,
        filters: [{ name: "DeskHub Backup", extensions: ["db"] }],
      });
      if (!dest) return;
      await dbExport(dest);
      message = "已导出备份 / Backup exported";
    } catch (e) {
      message = `导出失败 / Export failed: ${e}`;
    } finally {
      busy = false;
    }
  }

  async function importBackup() {
    busy = true;
    message = "";
    try {
      const src = await open({
        multiple: false,
        directory: false,
        filters: [{ name: "DeskHub Backup", extensions: ["db"] }],
      });
      if (!src || typeof src !== "string") return;
      await dbImport(src);
      message = "已导入，请重启应用以生效 / Imported — restart the app to apply";
    } catch (e) {
      message = `导入失败 / Import failed: ${e}`;
    } finally {
      busy = false;
    }
  }
</script>

<main class="container">
  <h1>设置 / Settings</h1>

  <section class="card">
    <h2>开机自启 / Autostart</h2>
    <label class="row">
      <input type="checkbox" checked={autostart} onchange={toggleAutostart} disabled={busy} />
      <span>随 Windows 启动（隐藏到托盘） / Start with Windows (hidden to tray)</span>
    </label>
  </section>

  <section class="card">
    <h2>数据备份 / Backup</h2>
    <p>导出当前数据为 .db 文件，或从备份恢复（导入后需重启）。</p>
    <p>Export your data as a .db file, or restore from a backup (restart after import).</p>
    <div class="row">
      <button onclick={exportBackup} disabled={busy}>导出备份 / Export</button>
      <button onclick={importBackup} disabled={busy}>导入备份 / Import</button>
    </div>
  </section>

  {#if message}
    <p class="msg">{message}</p>
  {/if}
</main>

<style>
  .container {
    max-width: 800px;
    margin: 0 auto;
    padding: 1.5rem 1rem;
  }

  .card {
    margin-bottom: 1.25rem;
    padding: 1rem;
    border: 1px solid var(--border);
    border-radius: 10px;
  }

  h2 {
    font-size: 1.05rem;
    margin: 0 0 0.6rem;
  }

  .row {
    display: flex;
    align-items: center;
    gap: 0.6rem;
  }

  button {
    border-radius: 8px;
    border: 1px solid var(--border);
    padding: 0.5em 0.9em;
    font-size: 1em;
    color: var(--fg);
    background: var(--surface);
    cursor: pointer;
  }

  .msg {
    opacity: 0.85;
  }
</style>
