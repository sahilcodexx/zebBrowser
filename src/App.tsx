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

export default function App() {
  const [input, setInput] = useState("")
  const centerRef = useRef<HTMLInputElement>(null)
  const [update, setUpdate] = useState<any>(null)
  const [updating, setUpdating] = useState(false)

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
    try {
      await invoke("navigate_browser", { url: u })
    } catch {
      window.location.href = u
    }
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

      {/* Center Spotlight Search */}
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
    </div>
  )
}
