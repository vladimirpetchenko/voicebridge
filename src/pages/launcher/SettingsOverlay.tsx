import { Check, RefreshCw, X } from "lucide-react";
import type {
  AppState,
  KnownDevice,
  MobileInfo,
  SttModelInfo,
} from "../../shared/types";
import { formatMb } from "../../shared/lib/format";

export const SETTINGS_TABS = [
  "Модели",
  "Микрофон",
  "OpenCode",
  "Горячие клавиши",
  "Мобильный доступ",
  "О программе",
] as const;

export interface SettingsOverlayProps {
  state: AppState;
  settingsTab: string;
  setSettingsTab: (tab: string) => void;
  onClose: () => void;
  onRefreshSettings: () => void;

  models: SttModelInfo[];
  downloads: Record<string, number>;
  anyDownloaded: boolean;
  onSelectModel: (id: string) => void;
  onDownloadModel: (id: string) => void;
  onRefreshModels: () => void;
  onSetLanguage: (lang: string) => void;

  microphones: string[];
  onSelectMicrophone: (name: string) => void;
  onRefreshMicrophones: () => void;
  onSetSensitivity: (value: number) => void;
  onSetSilenceTimeout: (seconds: number) => void;

  opencodeBinary: string;
  onSetSendMode: (mode: string) => void;
  onRefreshBinary: () => void;

  hotkeyDraft: string;
  setHotkeyDraft: (s: string) => void;
  onSaveHotkey: () => void;

  mobileInfo: MobileInfo | null;
  devices: KnownDevice[];
  onSetMobileEnabled: (enabled: boolean) => void;
  onRegenerateToken: () => void;
  onForgetDevice: (id: string) => void;

  updateStatus: string | null;
  updateChecking: boolean;
  onCheckUpdate: () => void;
}

/// Модальное окно настроек (вкладки).
export function SettingsOverlay(props: SettingsOverlayProps) {
  const {
    state,
    settingsTab,
    setSettingsTab,
    onClose,
    onRefreshSettings,
    models,
    downloads,
    anyDownloaded,
    onSelectModel,
    onDownloadModel,
    onRefreshModels,
    onSetLanguage,
    microphones,
    onSelectMicrophone,
    onRefreshMicrophones,
    onSetSensitivity,
    onSetSilenceTimeout,
    opencodeBinary,
    onSetSendMode,
    onRefreshBinary,
    hotkeyDraft,
    setHotkeyDraft,
    onSaveHotkey,
    mobileInfo,
    devices,
    onSetMobileEnabled,
    onRegenerateToken,
    onForgetDevice,
    updateStatus,
    updateChecking,
    onCheckUpdate,
  } = props;

  return (
    <div className="settings-overlay" onClick={onClose}>
      <div className="settings-panel" onClick={(e) => e.stopPropagation()}>
        <div className="settings-header">
          <h2>Настройки</h2>
          <div className="settings-header-actions">
            <button className="icon-btn" onClick={onRefreshSettings} title="Обновить">
              <RefreshCw size={15} />
            </button>
            <button className="icon-btn" onClick={onClose} title="Закрыть">
              <X size={16} />
            </button>
          </div>
        </div>
        <div className="settings-tabs">
          {SETTINGS_TABS.map((tab) => (
            <button
              key={tab}
              className={`settings-tab ${settingsTab === tab ? "active" : ""}`}
              onClick={() => setSettingsTab(tab)}
            >
              {tab}
            </button>
          ))}
        </div>

        {settingsTab === "Модели" && (
          <div className="settings-content">
            <label className="field">
              <span className="field-label">Язык распознавания</span>
              <select
                className="select"
                value={state.language}
                onChange={(e) => onSetLanguage(e.target.value)}
              >
                <option value="auto">Автоопределение</option>
                <option value="ru">Русский</option>
                <option value="en">Английский</option>
              </select>
            </label>

            {!anyDownloaded && (
              <div className="hint-banner">
                Скачайте модель, чтобы начать. Рекомендуем Whisper Base.
              </div>
            )}

            <div className="model-list">
              {models.map((m) => {
                const downloading = downloads[m.id] !== undefined;
                return (
                  <div key={m.id} className={`model-item ${!m.supported ? "disabled" : ""}`}>
                    <div className="model-info">
                      <label className="model-name">
                        <input
                          type="radio"
                          name="model"
                          checked={state.selectedModel === m.id}
                          disabled={!m.supported || !m.downloaded}
                          onChange={() => onSelectModel(m.id)}
                        />
                        <span>{m.name}</span>
                        <span className="model-size">{formatMb(m.sizeMb)}</span>
                      </label>
                      <span className="model-desc">{m.description}</span>
                    </div>
                    <div className="model-action">
                      {!m.supported ? (
                        <span className="badge-soon">Скоро</span>
                      ) : m.downloaded ? (
                        <span className="badge-done"><Check size={12} /> Скачана</span>
                      ) : downloading ? (
                        <div className="progress">
                          <div
                            className="progress-fill"
                            style={{ width: `${Math.min(100, downloads[m.id])}%` }}
                          />
                          <span className="progress-text">
                            {Math.round(downloads[m.id])}%
                          </span>
                        </div>
                      ) : (
                        <button className="btn small" onClick={() => onDownloadModel(m.id)}>
                          Скачать
                        </button>
                      )}
                    </div>
                  </div>
                );
              })}
            </div>

            <button className="btn" onClick={onRefreshModels}>
              <RefreshCw size={14} /> Обновить
            </button>
          </div>
        )}

        {settingsTab === "Микрофон" && (
          <div className="settings-content">
            <label className="field">
              <span className="field-label">Устройство ввода</span>
              <div className="field-row">
                <select
                  className="select"
                  value={state.selectedMicrophone ?? ""}
                  onChange={(e) => onSelectMicrophone(e.target.value)}
                >
                  <option value="">По умолчанию</option>
                  {microphones.map((m) => (
                    <option key={m} value={m}>
                      {m}
                    </option>
                  ))}
                </select>
                <button className="icon-btn" onClick={onRefreshMicrophones} title="Обновить список">
                  <RefreshCw size={15} />
                </button>
              </div>
            </label>

            <label className="field">
              <span className="field-label">
                Чувствительность: {state.sensitivity.toFixed(1)}×
              </span>
              <input
                type="range"
                min={0.1}
                max={5}
                step={0.1}
                value={state.sensitivity}
                onChange={(e) => onSetSensitivity(parseFloat(e.target.value))}
              />
            </label>

            <label className="field">
              <span className="field-label">Авто-остановка при тишине</span>
              <select
                className="select"
                value={state.silenceTimeout}
                onChange={(e) => onSetSilenceTimeout(parseFloat(e.target.value))}
              >
                <option value={0}>Выключена</option>
                <option value={2}>2 секунды</option>
                <option value={3}>3 секунды</option>
                <option value={5}>5 секунд</option>
              </select>
            </label>
          </div>
        )}

        {settingsTab === "OpenCode" && (
          <div className="settings-content">
            <label className="field">
              <span className="field-label">Режим отправки распознанного текста</span>
              <select
                className="select"
                value={state.sendMode}
                onChange={(e) => onSetSendMode(e.target.value)}
              >
                <option value="direct">Сразу в чат (разговор)</option>
                <option value="confirm">Предпросмотр перед отправкой</option>
              </select>
            </label>

            <label className="field">
              <span className="field-label">Исполняемый файл opencode</span>
              <div className="field-row">
                <input className="select" value={opencodeBinary} readOnly />
                <button className="icon-btn" onClick={onRefreshBinary} title="Пересканировать">
                  <RefreshCw size={15} />
                </button>
              </div>
            </label>
            <div className="hint-banner">
              Запущенные серверы OpenCode находятся автоматически (по процессам
              и портам 4096/12000/3000/17000) и обновляются каждые 5 секунд.
            </div>
          </div>
        )}

        {settingsTab === "Горячие клавиши" && (
          <div className="settings-content">
            <label className="field">
              <span className="field-label">Запись (глобально)</span>
              <div className="field-row">
                <input
                  className="select"
                  value={hotkeyDraft}
                  onChange={(e) => setHotkeyDraft(e.target.value)}
                  placeholder="Cmd+Shift+V"
                  spellCheck={false}
                />
                <button className="btn small" onClick={onSaveHotkey}>
                  Сохранить
                </button>
              </div>
            </label>
            <div className="hint-banner">
              Примеры: Cmd+Shift+V (macOS), Ctrl+Shift+V (Windows/Linux),
              Alt+Space. После сохранения комбинация применяется сразу.
            </div>
          </div>
        )}

        {settingsTab === "Мобильный доступ" && (
          <div className="settings-content">
            <label className="field">
              <span className="field-label">Принимать команды с мобильного</span>
              <input
                type="checkbox"
                checked={state.mobileEnabled}
                onChange={(e) => onSetMobileEnabled(e.target.checked)}
              />
            </label>

            {state.mobileEnabled && mobileInfo && (
              <>
                <div className="field">
                  <span className="field-label">QR-код для подключения</span>
                  <div
                    className="mobile-qr"
                    dangerouslySetInnerHTML={{ __html: mobileInfo.qrSvg }}
                  />
                </div>
                <div className="field">
                  <span className="field-label">Адрес</span>
                  <span className="hint-banner" style={{ wordBreak: "break-all" }}>
                    {mobileInfo.uri}
                  </span>
                </div>
                <button className="btn" onClick={onRegenerateToken}>
                  Перевыпустить токен
                </button>

                <div className="field">
                  <span className="field-label">Устройства</span>
                  {devices.length === 0 ? (
                    <span className="hint-banner">Пока нет подключённых устройств.</span>
                  ) : (
                    <div className="device-list">
                      {devices.map((d) => (
                        <div className="device-row" key={d.id}>
                          <div className="device-meta">
                            <span className="device-name">{d.name}</span>
                            <span className="device-seen">
                              подключено {new Date(d.lastSeen * 1000).toLocaleString()}
                            </span>
                          </div>
                          <button className="btn" onClick={() => onForgetDevice(d.id)}>
                            Забыть
                          </button>
                        </div>
                      ))}
                    </div>
                  )}
                </div>
              </>
            )}

            <div className="hint-banner">
              Мобильное приложение подключается по локальной сети и управляет
              десктопом. Для доступа из другой сети используйте Tailscale/ZeroTier.
            </div>
          </div>
        )}

        {settingsTab === "О программе" && (
          <div className="settings-content">
            <p>VoiceBridge — голосовой ассистент для разработчиков.</p>
            <p>Локальное распознавание речи (whisper.cpp), управление OpenCode.</p>
            <p className="about-version">Версия 0.1.0 · Rust + Tauri · React</p>
            <p>
              <a
                className="link"
                href="https://github.com/vladimirpetchenko/voicebridge"
                target="_blank"
                rel="noreferrer"
              >
                github.com/vladimirpetchenko/voicebridge
              </a>
            </p>
            <button
              className="btn"
              onClick={onCheckUpdate}
              disabled={updateChecking}
            >
              <RefreshCw size={14} /> Проверить обновления
            </button>
            {updateStatus && <p className="about-version">{updateStatus}</p>}
          </div>
        )}

        <button className="btn" onClick={onClose}>
          Закрыть
        </button>
      </div>
    </div>
  );
}
