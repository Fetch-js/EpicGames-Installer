import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { open } from "@tauri-apps/plugin-dialog";
import { createIcons, Check, FolderOpen, Maximize2, Minus, X } from "lucide";
import epicLogo from "../Group 17.png";
import "./style.css";

const appWindow = getCurrentWindow();

document.querySelector("#app").innerHTML = `
  <main class="installer-shell">
    <header class="titlebar" data-tauri-drag-region>
      <div class="brand" data-tauri-drag-region>
        <img class="brand-mark" src="${epicLogo}" alt="" />
        <span data-tauri-drag-region>Epic Games Launcher Setup</span>
      </div>
      <nav class="window-controls" aria-label="Управление окном">
        <button id="minimize" class="window-button" aria-label="Свернуть" title="Свернуть"><i data-lucide="minus"></i></button>
        <button id="maximize" class="window-button" aria-label="Развернуть" title="Развернуть"><i data-lucide="maximize-2"></i></button>
        <button id="close" class="window-button close" aria-label="Закрыть" title="Закрыть"><i data-lucide="x"></i></button>
      </nav>
    </header>

    <section class="workspace">
      <aside class="visual" aria-hidden="true">
        <div class="visual-line line-one"></div>
        <div class="visual-line line-two"></div>
        <div class="tile tile-small"></div>
        <div class="tile tile-large"></div>
        <img class="hero-logo" src="${epicLogo}" alt="Epic Games" />
        <span class="credit">by fetch.js</span>
      </aside>

      <section class="setup-panel">
        <div id="step-label" class="step-label">ШАГ 1 ИЗ 2</div>
        <h1 id="setup-title">Место установки</h1>
        <p id="setup-description" class="lead">Выберите папку, в которую будет установлен Epic Games Launcher.</p>

        <label class="field-label" for="install-path">Папка назначения</label>
        <div class="path-field">
          <input id="install-path" type="text" spellcheck="false" value="C:\\Program Files\\Epic Games\\Launcher" />
          <button id="browse" class="icon-button" type="button" aria-label="Выбрать папку" title="Выбрать папку">
            <i data-lucide="folder-open"></i>
          </button>
        </div>

        <div class="requirements">
          <div><span>Требуется</span><strong id="required-space">Вычисление...</strong></div>
          <div><span>Доступно</span><strong id="available-space">Сканирование...</strong></div>
        </div>

        <label class="checkbox-row">
          <input id="shortcut" type="checkbox" checked />
          <span class="checkbox"><i data-lucide="check"></i></span>
          <span>Создать ярлык на рабочем столе</span>
        </label>

        <div id="progress-area" class="progress-area" hidden>
          <div class="progress-copy"><span id="progress-status">Подготовка...</span><strong id="progress-value">0%</strong></div>
          <div class="progress-track"><div id="progress-bar"></div></div>
        </div>

        <p id="message" class="message" role="status" aria-live="polite"></p>

        <footer class="actions">
          <button id="install" class="primary-button" type="button">Установить</button>
        </footer>
      </section>
    </section>
  </main>
`;

createIcons({ icons: { Check, FolderOpen, Maximize2, Minus, X } });

const elements = {
  path: document.querySelector("#install-path"),
  browse: document.querySelector("#browse"),
  install: document.querySelector("#install"),
  message: document.querySelector("#message"),
  progressArea: document.querySelector("#progress-area"),
  progressBar: document.querySelector("#progress-bar"),
  progressValue: document.querySelector("#progress-value"),
  progressStatus: document.querySelector("#progress-status"),
  requiredSpace: document.querySelector("#required-space"),
  availableSpace: document.querySelector("#available-space"),
  stepLabel: document.querySelector("#step-label"),
  setupTitle: document.querySelector("#setup-title"),
  setupDescription: document.querySelector("#setup-description"),
};

let spaceScanTimer;
let hasEnoughSpace = false;

await listen("install-progress", ({ payload }) => {
  animateProgress(payload.percent, payload.status);
});

document.querySelector("#minimize").addEventListener("click", () => appWindow.minimize());
document.querySelector("#maximize").addEventListener("click", () => appWindow.toggleMaximize());
document.querySelector("#close").addEventListener("click", () => appWindow.close());

elements.browse.addEventListener("click", async () => {
  const selected = await open({ directory: true, multiple: false, title: "Выберите папку установки" });
  if (selected) {
    elements.path.value = selected;
    await scanDiskSpace();
  }
});

elements.path.addEventListener("input", () => {
  clearTimeout(spaceScanTimer);
  elements.availableSpace.textContent = "Сканирование...";
  spaceScanTimer = setTimeout(scanDiskSpace, 350);
});

elements.install.addEventListener("click", async () => {
  const destination = elements.path.value.trim();
  if (!destination) {
    showMessage("Укажите папку для установки.", true);
    elements.path.focus();
    return;
  }
  if (!hasEnoughSpace) {
    showMessage("На выбранном диске недостаточно свободного места.", true);
    return;
  }

  setInstalling(true);
  elements.stepLabel.textContent = "ШАГ 2 ИЗ 2";
  elements.setupTitle.textContent = "Установка";
  elements.setupDescription.textContent = "Подождите, пока файлы Epic Games Launcher будут установлены.";
  animateProgress(1, "Подготовка загрузки...");

  try {
    const result = await invoke("install_launcher", {
      destination,
      createShortcut: document.querySelector("#shortcut").checked,
    });
    animateProgress(100, "Установка завершена");
    elements.setupTitle.textContent = "Готово";
    elements.setupDescription.textContent = "Epic Games Launcher успешно установлен на компьютер.";
    showMessage(result, false);
    elements.install.textContent = "Готово";
    elements.install.disabled = false;
    elements.install.onclick = () => appWindow.close();
  } catch (error) {
    animateProgress(0, "Установка прервана");
    elements.stepLabel.textContent = "ШАГ 1 ИЗ 2";
    elements.setupTitle.textContent = "Место установки";
    elements.setupDescription.textContent = "Проверьте папку назначения и повторите установку.";
    showMessage(String(error), true);
    setInstalling(false);
    await scanDiskSpace();
  }
});

function animateProgress(value, status) {
  elements.progressArea.hidden = false;
  elements.progressBar.style.width = `${value}%`;
  elements.progressValue.textContent = `${value}%`;
  elements.progressStatus.textContent = status;
}

function setInstalling(active) {
  elements.install.disabled = active;
  elements.browse.disabled = active;
  elements.path.disabled = active;
  if (active) showMessage("", false);
}

function showMessage(text, isError) {
  elements.message.textContent = text;
  elements.message.classList.toggle("error", isError);
}

async function scanDiskSpace() {
  const destination = elements.path.value.trim();
  if (!destination) {
    hasEnoughSpace = false;
    elements.availableSpace.textContent = "Путь не указан";
    return;
  }

  try {
    const space = await invoke("get_disk_space", { destination });
    elements.requiredSpace.textContent = formatBytes(space.requiredBytes);
    elements.availableSpace.textContent = formatBytes(space.availableBytes);
    hasEnoughSpace = space.availableBytes >= space.requiredBytes;
    elements.availableSpace.classList.toggle("insufficient", !hasEnoughSpace);
    elements.install.disabled = !hasEnoughSpace;
  } catch (error) {
    hasEnoughSpace = false;
    elements.availableSpace.textContent = "Не удалось определить";
    elements.availableSpace.classList.add("insufficient");
    elements.install.disabled = true;
  }
}

function formatBytes(bytes) {
  const units = ["Б", "КБ", "МБ", "ГБ", "ТБ"];
  let value = Number(bytes);
  let unit = 0;
  while (value >= 1024 && unit < units.length - 1) {
    value /= 1024;
    unit += 1;
  }
  const precision = value >= 100 || unit === 0 ? 0 : 1;
  return `${value.toFixed(precision)} ${units[unit]}`;
}

scanDiskSpace();
