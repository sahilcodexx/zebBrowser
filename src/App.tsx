import { useState, useRef, useEffect } from "react"
import { invoke } from "@tauri-apps/api/core"
import { listen } from "@tauri-apps/api/event"
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
function hostFromUrl(u: string) {
  try { return new URL(u).hostname.replace(/^www\./, "") } catch { return u }
}

const isTauriApp = typeof window !== "undefined" && ("__TAURI__" in window || "__TAURI_INTERNALS__" in window)

export default function App() {
  const [url, setUrl] = useState<string | null>(null)
  const [input, setInput] = useState("")
  const [topVisible, setTopVisible] = useState(false)
  const centerRef = useRef<HTMLInputElement>(null)
  const topRef = useRef<HTMLInputElement>(null)
  const iframeRef = useRef<HTMLIFrameElement>(null)
  const isStart = url === null
  const [update, setUpdate] = useState<any>(null)
  const [updating, setUpdating] = useState(false)

  useEffect(() => { if (isStart) centerRef.current?.focus() }, [isStart])

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

  // global shortcut events from Rust + normal shortcuts
  useEffect(() => {
    const unlistenFns: (() => void)[] = []
    listen("focus-url", () => {
      if (isStart) { centerRef.current?.focus(); centerRef.current?.select() }
      else { setTopVisible(true); setTimeout(()=>{topRef.current?.focus(); topRef.current?.select()}, 30) }
    }).then(fn => unlistenFns.push(fn))
    listen("reload-page", () => {
      if (isTauriApp) {
        invoke("browser_reload").catch(()=>{})
      } else {
        try { (iframeRef.current?.contentWindow as any)?.location.reload() } catch { if(url) setUrl(u=>u) }
      }
    }).then(fn => unlistenFns.push(fn))
    listen("go-back", () => {
      if (isTauriApp) {
        invoke("browser_go_back").catch(()=>{})
      } else {
        try { (iframeRef.current?.contentWindow as any)?.history.back() } catch {}
      }
    }).then(fn => unlistenFns.push(fn))
    listen("go-forward", () => {
      if (isTauriApp) {
        invoke("browser_go_forward").catch(()=>{})
      } else {
        try { (iframeRef.current?.contentWindow as any)?.history.forward() } catch {}
      }
    }).then(fn => unlistenFns.push(fn))
    listen("hide-bars", () => {
      setTopVisible(false)
      ;(document.activeElement as HTMLElement)?.blur()
    }).then(fn => unlistenFns.push(fn))

    const onKey = (e: KeyboardEvent) => {
      const mod = e.ctrlKey || e.metaKey
      if (mod && e.key.toLowerCase() === "l") {
        e.preventDefault()
        if (isStart) { centerRef.current?.focus(); centerRef.current?.select() }
        else { setTopVisible(true); setTimeout(()=>{topRef.current?.focus(); topRef.current?.select()}, 30) }
      }
      if (mod && e.key.toLowerCase() === "r") {
        e.preventDefault()
        if (isTauriApp) {
          invoke("browser_reload").catch(()=>{})
        } else {
          try { (iframeRef.current?.contentWindow as any)?.location.reload() } catch { if(url) setUrl(u=>u) }
        }
      }
      if (e.key === "F5") {
        e.preventDefault()
        if (isTauriApp) {
          invoke("browser_reload").catch(()=>{})
        } else {
          try { (iframeRef.current?.contentWindow as any)?.location.reload() } catch {}
        }
      }
      if (e.altKey && e.key === "ArrowLeft") {
        e.preventDefault()
        if (isTauriApp) {
          invoke("browser_go_back").catch(()=>{})
        } else {
          try { (iframeRef.current?.contentWindow as any)?.history.back() } catch {}
        }
      }
      if (e.altKey && e.key === "ArrowRight") {
        e.preventDefault()
        if (isTauriApp) {
          invoke("browser_go_forward").catch(()=>{})
        } else {
          try { (iframeRef.current?.contentWindow as any)?.history.forward() } catch {}
        }
      }
      if (e.key === "Escape") {
        setTopVisible(false)
        ;(document.activeElement as HTMLElement)?.blur()
      }
    }
    window.addEventListener("keydown", onKey)
    return () => {
      window.removeEventListener("keydown", onKey)
      unlistenFns.forEach(fn=>fn())
    }
  }, [isStart, url])

  const navigate = async (raw: string) => {
    const u = makeUri(raw)
    if (!u) return
    setUrl(u)
    setInput(u)
    setTopVisible(false)
    setTimeout(()=>{ centerRef.current?.blur(); topRef.current?.blur(); (document.activeElement as HTMLElement)?.blur() }, 10)

    if (isTauriApp) {
      try {
        await invoke("navigate_browser", { url: u })
      } catch (e) {
        console.error("navigate_browser failed:", e)
      }
    }
  }

  const goHome = async () => {
    setUrl(null)
    setInput("")
    setTopVisible(false)
    if (isTauriApp) {
      try {
        await invoke("close_browser")
      } catch (e) {}
    }
  }

  const onMouseMove = (e: React.MouseEvent) => {
    if (isStart) return
    const y = e.clientY
    if (y < 20) setTopVisible(true)
    else if (y > 56) {
      if (document.activeElement !== topRef.current) setTopVisible(false)
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
    <div className="app" onMouseMove={onMouseMove}>
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
      <div className={`top-bar ${topVisible && !isStart ? "visible" : ""}`}>
        <div className="top-inner">
          <div className="title-pill">{url ? hostFromUrl(url) : ""}</div>
          <input
            ref={topRef}
            className="top-entry"
            value={input}
            onChange={(e) => setInput(e.target.value)}
            onKeyDown={(e) => { if (e.key === "Enter") navigate(input) }}
            onFocus={(e) => e.currentTarget.select()}
            onBlur={() => setTimeout(()=>{ if(!isStart) setTopVisible(false)}, 150)}
            placeholder="Search or enter address"
            spellCheck={false}
          />
        </div>
      </div>
      {!isStart && !topVisible && <div className="hover-handle" onMouseEnter={()=>setTopVisible(true)} />}

      <div className={`center-wrap ${isStart ? "start" : "browsing"}`}>
        <div className="center-box">
          <input
            ref={centerRef}
            className="center-entry"
            value={input}
            onChange={(e) => setInput(e.target.value)}
            onKeyDown={(e) => { if (e.key === "Enter") navigate(input) }}
            placeholder="Enter URL or search..."
            spellCheck={false}
            autoFocus={isStart}
            onFocus={(e) => e.currentTarget.select()}
          />
        </div>
        {isStart && <div className="hint">Enter to search · Ctrl+L top bar · Ctrl+R reload · Alt←→ back/forward · F5 reload</div>}
      </div>

      <div className="web-wrap">
        {isStart ? (
          <div className="blank" />
        ) : !isTauriApp ? (
          <iframe
            ref={iframeRef}
            key={url}
            className="webview"
            src={url!}
            title="web"
            sandbox="allow-scripts allow-same-origin allow-forms allow-popups allow-top-navigation allow-downloads"
            allow="clipboard-read; clipboard-write; fullscreen; autoplay; encrypted-media"
            referrerPolicy="no-referrer-when-downgrade"
          />
        ) : null}
      </div>

      {!isStart && (
        <button className="home-btn" onClick={goHome} title="Home">
          ⌘
        </button>
      )}
    </div>
  )
}
