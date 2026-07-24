import "./style.css";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import { openUrl } from "@tauri-apps/plugin-opener";
import { check } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";

// Détecte si on tourne bien dans Tauri (sinon : aperçu navigateur, mode dégradé).
const IN_TAURI = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

const el = (id) => document.getElementById(id);
const state = {
  config: null,
  settings: { nickname: "", gta_path: null },
  gameReady: false,
  updating: false,
  needsUpdate: false,
  launcherUpdating: false,
  lastUpdateCheck: 0,
  updateNoticeSignature: null,
};

const noticeState = {
  currentKey: null,
  persistent: null,
  timeoutId: null,
  transitionId: null,
};

/* ---------------- Utilitaires ---------------- */

function renderNotice(notice) {
  clearTimeout(noticeState.transitionId);
  const box = el("activity-status");
  box.classList.remove("is-leaving");
  box.dataset.kind = notice.kind;
  box.setAttribute("role", notice.kind === "error" ? "alert" : "status");
  const message = el("activity-status-message");
  message.textContent = notice.message;
  message.title = notice.message;
  noticeState.currentKey = notice.key;
  box.hidden = false;
}

function hideNotice() {
  clearTimeout(noticeState.transitionId);
  const box = el("activity-status");
  if (box.hidden) return;
  box.classList.add("is-leaving");
  noticeState.transitionId = setTimeout(() => {
    box.hidden = true;
    box.classList.remove("is-leaving");
    noticeState.currentKey = null;
  }, 180);
}

function showPersistentNoticeOrHide() {
  if (noticeState.persistent) {
    renderNotice(noticeState.persistent);
  } else {
    hideNotice();
  }
}

function toast(message, kind = "info", timeout = 4000, key = null) {
  clearTimeout(noticeState.timeoutId);
  const notice = { message, kind, key: key || `${kind}:${message}` };

  if (timeout <= 0) {
    noticeState.persistent = notice;
    renderNotice(notice);
    return;
  }

  renderNotice(notice);
  noticeState.timeoutId = setTimeout(showPersistentNoticeOrHide, timeout);
}

function clearNotice(key = null) {
  if (!key || noticeState.persistent?.key === key) {
    noticeState.persistent = null;
  }
  if (!key || noticeState.currentKey === key) {
    clearTimeout(noticeState.timeoutId);
    showPersistentNoticeOrHide();
  }
}

function dismissCurrentNotice() {
  clearTimeout(noticeState.timeoutId);
  if (noticeState.currentKey === noticeState.persistent?.key) {
    noticeState.persistent = null;
    hideNotice();
    return;
  }
  showPersistentNoticeOrHide();
}

async function call(cmd, args) {
  if (!IN_TAURI) {
    console.warn(`[aperçu] commande ignorée : ${cmd}`);
    throw new Error("Aperçu navigateur : backend indisponible");
  }
  return invoke(cmd, args);
}

function isValidNickname(nick) {
  if (nick.length < 3 || nick.length > 24) return false;
  return /^[A-Za-z0-9\[\]();$=@._-]+$/.test(nick);
}

function fmtBytes(n) {
  if (!n) return "0 o";
  const u = ["o", "Ko", "Mo", "Go"];
  const i = Math.floor(Math.log(n) / Math.log(1024));
  return `${(n / Math.pow(1024, i)).toFixed(1)} ${u[i]}`;
}

/* ---------------- Rendu ---------------- */

function renderStatus(status) {
  const pill = el("status-pill");
  if (!status || !status.online) {
    pill.dataset.state = "offline";
    el("status-label").textContent = "Hors ligne";
    el("status-players").textContent = "—";
    el("status-ping").textContent = "";
    return;
  }
  pill.dataset.state = "online";
  el("status-label").textContent = "En ligne";
  el("status-players").textContent = `${status.players}/${status.max_players} joueurs`;
  el("status-ping").textContent = status.ping_ms ? `${status.ping_ms} ms` : "";
}

function renderGamePath() {
  const value = el("game-path-value");
  if (state.settings.gta_path) {
    value.textContent = state.settings.gta_path;
    value.classList.remove("missing");
    state.gameReady = true;
  } else {
    value.textContent = "Introuvable — clique sur Modifier";
    value.classList.add("missing");
    state.gameReady = false;
  }
  updatePlayButton();
}

function renderNews(feed) {
  const list = el("news-list");
  const items = (feed && feed.items) || [];
  if (items.length === 0) {
    list.innerHTML = `<div class="news-empty">Aucune actualité pour le moment.</div>`;
    return;
  }
  list.innerHTML = "";
  for (const item of items) {
    const div = document.createElement("div");
    div.className = "news-item";
    div.innerHTML = `
      <div class="n-top">
        <span class="n-title"></span>
        ${item.tag ? `<span class="n-tag"></span>` : ""}
      </div>
      ${item.date ? `<div class="n-date"></div>` : ""}
      ${item.body ? `<div class="n-body"></div>` : ""}
    `;
    div.querySelector(".n-title").textContent = item.title || "";
    if (item.tag) div.querySelector(".n-tag").textContent = item.tag;
    if (item.date) div.querySelector(".n-date").textContent = item.date;
    if (item.body) div.querySelector(".n-body").textContent = item.body;
    list.appendChild(div);
  }
}

function updatePlayButton() {
  const btn = el("play-btn");
  const label = el("play-label");
  const sub = el("play-sub");
  const nickOk = isValidNickname(el("nickname").value.trim());

  if (state.updating) {
    btn.disabled = true;
    btn.classList.add("busy");
    label.textContent = "MISE À JOUR…";
    sub.textContent = "";
    return;
  }
  btn.classList.remove("busy");

  if (!state.gameReady) {
    btn.disabled = true;
    label.textContent = "JOUER";
    sub.textContent = "Jeu introuvable";
    return;
  }
  if (!nickOk) {
    btn.disabled = true;
    label.textContent = "JOUER";
    sub.textContent = "Choisis un pseudo";
    return;
  }
  btn.disabled = false;
  label.textContent = state.needsUpdate ? "METTRE À JOUR & JOUER" : "JOUER";
  sub.textContent = state.needsUpdate ? "Mise à jour requise" : "";
}

function setProgress(done, total, file) {
  const wrap = el("progress");
  const bar = el("progress-bar");
  const text = el("progress-text");
  wrap.hidden = false;
  const pct = total > 0 ? Math.min(100, (done / total) * 100) : 0;
  bar.style.width = `${pct}%`;
  text.textContent = file
    ? `${file} — ${fmtBytes(done)} / ${fmtBytes(total)}`
    : `${fmtBytes(done)} / ${fmtBytes(total)}`;
}

/* ---------------- Actions ---------------- */

async function refreshStatus() {
  try {
    const status = await call("get_server_status");
    renderStatus(status);
  } catch (e) {
    renderStatus(null);
  }
}

async function refreshNews() {
  try {
    const feed = await call("get_news");
    renderNews(feed);
  } catch (e) {
    renderNews(null);
  }
}

async function detectGame() {
  if (state.settings.gta_path) return;
  try {
    const install = await call("detect_game");
    if (install && install.gta_exe) {
      await call("set_game_path", { gtaExe: install.gta_exe });
      state.settings.gta_path = install.gta_exe;
    }
  } catch (e) {
    /* détection silencieuse */
  }
  renderGamePath();
}

async function browseGame() {
  try {
    const selected = await openDialog({
      title: "Sélectionne gta_sa.exe",
      multiple: false,
      filters: [{ name: "GTA San Andreas", extensions: ["exe"] }],
    });
    if (!selected) return;
    const install = await call("set_game_path", { gtaExe: selected });
    state.settings.gta_path = install.gta_exe;
    renderGamePath();
    toast("Jeu configuré avec succès.", "success");
    checkUpdates();
  } catch (e) {
    toast(`${e}`, "error");
  }
}

async function saveNickname() {
  const nick = el("nickname").value.trim();
  const input = el("nickname");
  if (!isValidNickname(nick)) {
    input.classList.add("invalid");
    updatePlayButton();
    return;
  }
  input.classList.remove("invalid");
  try {
    await call("set_nickname", { nickname: nick });
    state.settings.nickname = nick;
  } catch (e) {
    toast(`${e}`, "error");
  }
  updatePlayButton();
}

async function checkUpdates() {
  if (!state.gameReady) return;
  try {
    const plan = await call("check_updates");
    state.needsUpdate = !plan.up_to_date;
    if (state.needsUpdate) {
      const signature = `${plan.files.length}:${plan.total_bytes}`;
      if (signature !== state.updateNoticeSignature) {
        toast(
          `${plan.files.length} fichier(s) à mettre à jour (${fmtBytes(plan.total_bytes)}).`,
          "info",
          0,
          "modpack-update",
        );
        state.updateNoticeSignature = signature;
      }
    } else {
      state.updateNoticeSignature = null;
      clearNotice("modpack-update");
    }
  } catch (e) {
    // Pas bloquant : on laisse jouer même si le manifest est injoignable.
    console.warn("check_updates:", e);
  }
  updatePlayButton();
}

async function runUpdatesThenLaunch() {
  // Re-vérifie l'état du modpack juste avant de jouer : évite de lancer avec un
  // modpack périmé si le launcher est resté ouvert pendant une mise à jour.
  await checkUpdates();

  if (state.needsUpdate) {
    state.updating = true;
    clearNotice("modpack-update");
    updatePlayButton();
    try {
      await call("apply_updates");
      state.needsUpdate = false;
      state.updateNoticeSignature = null;
      el("progress").hidden = true;
      toast("Mise à jour terminée.", "success");
    } catch (e) {
      state.updating = false;
      state.updateNoticeSignature = null;
      el("progress").hidden = true;
      updatePlayButton();
      toast(`Échec de la mise à jour : ${e}`, "error");
      return;
    }
    state.updating = false;
    updatePlayButton();
  }

  // Précharge tous les modèles/textures déclarés par le serveur dans le cache
  // natif SA-MP. En cas d'indisponibilité du CDN, SA-MP conserve son propre
  // téléchargement à la connexion comme filet de sécurité.
  await preloadSampCache();
  await launch();
}

async function preloadSampCache() {
  state.updating = true;
  updatePlayButton();
  try {
    const result = await call("sync_samp_cache");
    el("progress").hidden = true;
    if (result.downloaded_files > 0) {
      const mib = (result.bytes_downloaded / 1024 / 1024).toFixed(1);
      toast(
        `Cache SA-MP prêt : ${result.downloaded_files} fichier(s) ajouté(s), ${mib} Mio.`,
        "success",
        7000,
      );
    }
  } catch (e) {
    el("progress").hidden = true;
    console.warn("sync_samp_cache:", e);
    toast(
      `Préchargement du cache indisponible — SA-MP prendra le relais : ${e}`,
      "info",
      9000,
    );
  } finally {
    state.updating = false;
    updatePlayButton();
  }
}

async function launch() {
  state.updating = true;
  updatePlayButton();
  toast(
    "Contrôle intégral et authentification de l’installation GTRP…",
    "info",
    0,
    "integrity-check",
  );
  try {
    const graphics = await call("launch_game");
    clearNotice("integrity-check");
    if (graphics?.message) {
      const kind = graphics.applied ? "success" : "info";
      toast(graphics.message, kind);
    } else {
      toast("Lancement du jeu…", "success");
    }
  } catch (e) {
    clearNotice("integrity-check");
    toast(`${e}`, "error");
  } finally {
    state.updating = false;
    updatePlayButton();
  }
}

async function saveEnhancedGraphics() {
  const enabled = el("enhanced-graphics").checked;
  try {
    await call("set_enhanced_graphics", { enabled });
    state.settings.enhanced_graphics = enabled;
  } catch (e) {
    toast(`${e}`, "error");
  }
}

async function checkLauncherUpdate() {
  if (!IN_TAURI || state.launcherUpdating) return;
  try {
    const update = await check();
    if (!update) return;

    state.launcherUpdating = true;
    toast(`Mise à jour du launcher ${update.version}…`, "info", 12000);

    let downloaded = 0;
    let total = 0;
    await update.downloadAndInstall((event) => {
      if (event.event === "Started" && event.data?.contentLength) {
        total = event.data.contentLength;
        setProgress(0, total, "Mise à jour du launcher");
      } else if (event.event === "Progress") {
        downloaded += event.data.chunkLength;
        setProgress(downloaded, total || downloaded, "Mise à jour du launcher");
      } else if (event.event === "Finished") {
        el("progress").hidden = true;
      }
    });

    toast("Redémarrage du launcher…", "success", 3000);
    await relaunch();
  } catch (e) {
    state.launcherUpdating = false;
    console.warn("launcher update:", e);
  }
}

/* ---------------- Initialisation ---------------- */

async function init() {
  // Config serveur
  try {
    state.config = await call("get_config");
  } catch (e) {
    state.config = {
      server_name: "Grand Theft RolePlay",
      web_url: "https://gtrp.fr",
      discord_url: "https://discord.gg/gtrp",
      launcher_version: "aperçu",
    };
  }
  el("server-name").textContent = state.config.server_name;
  el("version").textContent = `v${state.config.launcher_version}`;

  // Réglages
  try {
    state.settings = await call("load_settings");
  } catch (e) {
    /* aperçu */
  }
  el("nickname").value = state.settings.nickname || "";
  el("enhanced-graphics").checked = state.settings.enhanced_graphics !== false;

  // Événements de progression de mise à jour
  if (IN_TAURI) {
    listen("update-progress", (event) => {
      const p = event.payload;
      setProgress(p.bytes_done, p.bytes_total, p.current_file);
    });
    listen("integrity-violation", (event) => {
      const issue = event.payload;
      toast(
        `${issue?.message || "Modification non autorisée détectée : la session a été fermée."}`,
        "error",
        0,
        "integrity-violation",
      );
    });
  }

  // Écouteurs UI
  el("nickname").addEventListener("input", () => updatePlayButton());
  el("nickname").addEventListener("change", saveNickname);
  el("nickname").addEventListener("blur", saveNickname);
  el("browse-game").addEventListener("click", browseGame);
  el("enhanced-graphics").addEventListener("change", saveEnhancedGraphics);
  el("play-btn").addEventListener("click", runUpdatesThenLaunch);
  el("activity-status-close").addEventListener("click", dismissCurrentNotice);
  el("refresh-news").addEventListener("click", refreshNews);
  el("btn-discord").addEventListener("click", () =>
    openUrl(state.config.discord_url).catch(() => {})
  );
  el("btn-web").addEventListener("click", () =>
    openUrl(state.config.web_url).catch(() => {})
  );

  renderGamePath();
  updatePlayButton();

  // Mise à jour automatique du launcher (avant le reste).
  await checkLauncherUpdate();

  // Chargements réseau
  await detectGame();
  refreshStatus();
  refreshNews();
  checkUpdates();

  // Rafraîchissement périodique du statut
  setInterval(refreshStatus, 30000);

  // Re-vérifie périodiquement les mises à jour (launcher + modpack) : plus besoin
  // de redémarrer le launcher pour qu'une nouvelle version soit détectée.
  setInterval(checkForUpdates, 5 * 60 * 1000);

  // Re-vérifie aussi dès que la fenêtre reprend le focus (retour d'alt-tab),
  // avec un throttle pour éviter les appels réseau à répétition.
  window.addEventListener("focus", () => {
    const now = Date.now();
    if (now - state.lastUpdateCheck < 30000) return;
    checkForUpdates();
  });
}

/** Vérifie à la fois la mise à jour du launcher et celle du modpack. */
async function checkForUpdates() {
  // On ne perturbe pas une mise à jour du modpack en cours (évite un
  // redémarrage du launcher au milieu d'un téléchargement).
  if (state.updating) return;
  state.lastUpdateCheck = Date.now();
  await checkLauncherUpdate();
  await checkUpdates();
}

window.addEventListener("DOMContentLoaded", init);
