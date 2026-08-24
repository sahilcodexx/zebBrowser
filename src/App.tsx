import { useState, useRef, useEffect } from "react"
import { invoke } from "@tauri-apps/api/core"
import { check } from "@tauri-apps/plugin-updater"
import { relaunch } from "@tauri-apps/plugin-process"
import "./App.css"

function makeUri(raw: string): string | null {
  const s = raw.trim()
  if (!s) return null
  if (s.includes("://") || s.startsWith("http") || s.includes(".") || s.startsWith("localhost")) {
    return s.includes("://") ? s : `https://${s}`
  }
  return `https://duckduckgo.com/?q=${encodeURIComponent(s)}`
}

const isTauriApp = typeof window !== "undefined" && ("__TAURI__" in window || "__TAURI_INTERNALS__" in window)

type PerfSettings = {
  hardware_accel: boolean
  content_blocker_disabled: boolean
  lite_mode: boolean
}

export default function App() {
  const [input, setInput] = useState("")
  const centerRef = useRef<HTMLInputElement>(null)
  const [update, setUpdate] = useState<any>(null)
  const [updating, setUpdating] = useState(false)
  const [loading, setLoading] = useState(false)
  const [showSettings, setShowSettings] = useState(false)
  const [settings, setSettings] = useState<PerfSettings | null>(null)

  useEffect(() => {
    centerRef.current?.focus()
  }, [])

  // updater: check on mount, show popup if new version pushed
  useEffect(() => {
    let cancelled = false
    async function run() {
      try {
        if (!isTauriApp) return
        const u = await check()
        if (!cancelled && u) setUpdate(u)
      } catch {}
    }
    run()
    const id = setInterval(run, 30 * 60 * 1000)
    return () => { cancelled = true; clearInterval(id) }
  }, [])

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const mod = e.ctrlKey || e.metaKey
      if (mod && e.key.toLowerCase() === "l") {
        e.preventDefault()
        centerRef.current?.focus()
        centerRef.current?.select()
      }
      if (e.key === "Escape") {
        centerRef.current?.blur()
      }
    }
    window.addEventListener("keydown", onKey)
    return () => window.removeEventListener("keydown", onKey)
  }, [])

  const navigate = async (raw: string) => {
    const u = makeUri(raw)
    if (!u) return
    setLoading(true)
    try {
      await invoke("navigate_browser", { url: u })
    } catch {
      window.location.href = u
    }
    // The webview replaces this React app on navigation, so setLoading(false)
    // is intentionally not called here — the loading overlay is meant to be
    // visible up until the new page starts painting.
  }

  const doUpdate = async () => {
    if (!update) return
    try {
      setUpdating(true)
      await update.downloadAndInstall()
      await relaunch()
    } catch (e) {
      console.error(e)
      setUpdating(false)
    }
  }

  const openSettings = async () => {
    try {
      const s = await invoke<PerfSettings>("get_perf_settings")
      setSettings(s)
    } catch {
      setSettings({ hardware_accel: false, content_blocker_disabled: false, lite_mode: false })
    }
    setShowSettings(true)
  }

  return (
    <div className="app">
      {update && (
        <div className="updater-backdrop">
          <div className="updater-card">
            <div className="updater-title">Update available — v{update.version}</div>
            <div className="updater-notes">{update.body || "New version pushed. Update now?"}</div>
            <div className="updater-actions">
              <button className="updater-btn primary" onClick={doUpdate} disabled={updating}>
                {updating ? "Updating…" : "Update & relaunch"}
              </button>
              <button className="updater-btn" onClick={()=>setUpdate(null)} disabled={updating}>Later</button>
            </div>
          </div>
        </div>
      )}

      {showSettings && settings && (
        <div className="settings-backdrop" onClick={() => setShowSettings(false)}>
          <div className="settings-card" onClick={(e) => e.stopPropagation()}>
            <div className="settings-title">Performance</div>

            <Row
              label="Hardware acceleration"
              sub="GPU render via DMA-BUF. Fastest, but can crash on broken Wayland/Mesa stacks."
              on={settings.hardware_accel}
              envVar="ZEB_HARDWARE_ACCEL=1"
            />
            <Row
              label="Content blocker"
              sub="Short-circuits analytics / ads (Sentry, Vercel, GA, Hotjar, …) on every page."
              on={!settings.content_blocker_disabled}
              offEnvVar="ZEB_DISABLE_CONTENT_BLOCKER=1"
            />
            <Row
              label="Lite mode"
              sub="Kills CSS animations, lazy-loads images, disables WebGL. Smoothest on heavy sites."
              on={settings.lite_mode}
              envVar="ZEB_LITE=1"
            />

            <div className="settings-foot">
              Set the env var(s) above before launching <code>zeb</code>, then restart to apply.
            </div>
            <button className="settings-close" onClick={() => setShowSettings(false)}>Close</button>
          </div>
        </div>
      )}

      {loading ? (
        <div className="loading-overlay">
          <div className="loading-spinner" />
          <div className="loading-text">Loading…</div>
        </div>
      ) : (
        <div className="center-wrap start">
          <div className="center-box">
            <input
              ref={centerRef}
              className="center-entry"
              value={input}
              onChange={(e) => setInput(e.target.value)}
              onKeyDown={(e) => { if (e.key === "Enter") navigate(input) }}
              placeholder="Enter URL or search..."
              spellCheck={false}
              autoFocus
              onFocus={(e) => e.currentTarget.select()}
            />
          </div>
          <div className="hint">Enter to search · Ctrl+L / Esc home · Ctrl+R / F5 reload · Alt←→ back/forward</div>
        </div>
      )}

      {!loading && (
        <button className="perf-chip" onClick={openSettings} title="Performance settings">?</button>
      )}
    </div>
  )
}

function Row({ label, sub, on, envVar, offEnvVar }: { label: string; sub: string; on: boolean; envVar?: string; offEnvVar?: string }) {
  return (
    <div className="settings-row">
      <div className="settings-row-text">
        <div className="settings-row-label">{label}</div>
        <div className="settings-row-sub">{sub}</div>
        {envVar && <div className="settings-row-env"><code>{envVar}</code>{offEnvVar && <> / <code>{offEnvVar}</code></>}</div>}
      </div>
      <span className={"pill " + (on ? "on" : "off")}>{on ? "ON" : "off"}</span>
    </div>
  )
}
