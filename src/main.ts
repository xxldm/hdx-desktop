import { invoke } from "@tauri-apps/api/core";
import "./styles.css";

type DesktopFlavor = "local" | "online" | "unspecified";

type CapabilityStatus = {
  id: string;
  name: string;
  scope: "cross-platform" | "windows-only" | "flavor-local" | "flavor-online";
  platform: string;
  state: "stub" | "planned" | "not-supported";
  webviewExposure: "command-only" | "none";
  description: string;
};

type DesktopStatus = {
  flavor: DesktopFlavor;
  flavorLabel: string;
  productName: string;
  platform: string;
  includesAllInOne: boolean;
  remoteEndpointRequired: boolean;
  localActor: string | null;
  localTokenExposedToWebview: boolean;
  capabilities: CapabilityStatus[];
  boundaryNotes: string[];
};

const app = document.querySelector<HTMLDivElement>("#app");

if (!app) {
  throw new Error("缺少应用挂载节点。");
}

const appRoot = app;

function formatBoolean(value: boolean): string {
  return value ? "是" : "否";
}

function stateText(state: CapabilityStatus["state"]): string {
  switch (state) {
    case "stub":
      return "空壳";
    case "planned":
      return "待实现";
    case "not-supported":
      return "不支持";
  }
}

function renderStatus(status: DesktopStatus): void {
  appRoot.innerHTML = `
    <header class="topbar">
      <div>
        <p class="eyebrow">${status.productName}</p>
        <h1>运行状态</h1>
      </div>
      <button id="refresh" class="refresh-button" type="button">刷新</button>
    </header>
    <main class="layout">
      <section class="summary" aria-label="Desktop 边界">
        <div class="metric">
          <span class="metric-label">Flavor</span>
          <strong>${status.flavorLabel}</strong>
        </div>
        <div class="metric">
          <span class="metric-label">平台</span>
          <strong>${status.platform}</strong>
        </div>
        <div class="metric">
          <span class="metric-label">包含 all-in-one</span>
          <strong>${formatBoolean(status.includesAllInOne)}</strong>
        </div>
        <div class="metric">
          <span class="metric-label">需要远端地址</span>
          <strong>${formatBoolean(status.remoteEndpointRequired)}</strong>
        </div>
        <div class="metric">
          <span class="metric-label">本机身份</span>
          <strong>${status.localActor ?? "无"}</strong>
        </div>
        <div class="metric">
          <span class="metric-label">WebView 持有本机 token</span>
          <strong>${formatBoolean(status.localTokenExposedToWebview)}</strong>
        </div>
      </section>

      <section class="panel" aria-label="能力状态">
        <div class="panel-heading">
          <h2>Capability</h2>
          <span>${status.capabilities.length} 项</span>
        </div>
        <div class="capability-list">
          ${status.capabilities
            .map(
              (capability) => `
                <article class="capability">
                  <div>
                    <h3>${capability.name}</h3>
                    <p>${capability.description}</p>
                  </div>
                  <dl>
                    <div>
                      <dt>范围</dt>
                      <dd>${capability.scope}</dd>
                    </div>
                    <div>
                      <dt>平台</dt>
                      <dd>${capability.platform}</dd>
                    </div>
                    <div>
                      <dt>状态</dt>
                      <dd>${stateText(capability.state)}</dd>
                    </div>
                    <div>
                      <dt>WebView</dt>
                      <dd>${capability.webviewExposure}</dd>
                    </div>
                  </dl>
                </article>
              `
            )
            .join("")}
        </div>
      </section>

      <aside class="panel" aria-label="边界记录">
        <div class="panel-heading">
          <h2>边界</h2>
        </div>
        <ul class="notes">
          ${status.boundaryNotes.map((note) => `<li>${note}</li>`).join("")}
        </ul>
      </aside>
    </main>
  `;

  document.querySelector<HTMLButtonElement>("#refresh")?.addEventListener("click", () => {
    void loadStatus();
  });
}

function renderLoading(): void {
  appRoot.innerHTML = `
    <div class="loading">
      <div>
        <p class="eyebrow">HDX Desktop</p>
        <h1>正在读取运行状态</h1>
      </div>
    </div>
  `;
}

function renderError(error: unknown): void {
  const message = error instanceof Error ? error.message : String(error);
  appRoot.innerHTML = `
    <div class="loading error">
      <div>
        <p class="eyebrow">HDX Desktop</p>
        <h1>无法读取运行状态</h1>
        <p>${message}</p>
      </div>
    </div>
  `;
}

async function loadStatus(): Promise<void> {
  renderLoading();

  try {
    const status = await invoke<DesktopStatus>("desktop_status");
    renderStatus(status);
  } catch (error) {
    renderError(error);
  }
}

void loadStatus();
