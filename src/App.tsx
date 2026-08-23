import { useState, useRef, useEffect } from "react"
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
        // only in Tauri
        if (!("__TAURI__" in window)) return
        const u = await check()
        if (!cancelled && u) setUpdate(u)
      } catch {}
    }
    run()
    // also check every 30m
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
      try { (iframeRef.current?.contentWindow as any)?.location.reload() } catch { if(url) setUrl(u=>u) }
    }).then(fn => unlistenFns.push(fn))
    listen("go-back", () => {
      try { (iframeRef.current?.contentWindow as any)?.history.back() } catch {}
    }).then(fn => unlistenFns.push(fn))
    listen("go-forward", () => {
      try { (iframeRef.current?.contentWindow as any)?.history.forward() } catch {}
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
        try { (iframeRef.current?.contentWindow as any)?.location.reload() } catch { if(url) setUrl(u=>u) }
      }
      if (e.key === "F5") {
        e.preventDefault()
        try { (iframeRef.current?.contentWindow as any)?.location.reload() } catch {}
      }
      if (e.altKey && e.key === "ArrowLeft") {
        e.preventDefault()
        try { (iframeRef.current?.contentWindow as any)?.history.back() } catch {}
      }
      if (e.altKey && e.key === "ArrowRight") {
        e.preventDefault()
        try { (iframeRef.current?.contentWindow as any)?.history.forward() } catch {}
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

  const navigate = (raw: string) => {
    const u = makeUri(raw)
    if (!u) return
    setUrl(u)
    setInput(u)
    setTopVisible(false)
    setTimeout(()=>{ centerRef.current?.blur(); topRef.current?.blur(); (document.activeElement as HTMLElement)?.blur() }, 10)
  }

  const onMouseMove = (e: React.MouseEvent) => {
    if (isStart) return
    const y = e.clientY
    if (y < 20) setTopVisible(true)
    else if (y > 56) {
      if (document.activeElement !== topRef.current) setTopVisible(false)
    }
  }

  const onIframeLoad = () => {
    try {
      const doc = iframeRef.current?.contentDocument
      const win = iframeRef.current?.contentWindow as any
      if (!doc || !win) return
      doc.querySelectorAll('a[target="_blank"]').forEach((a) => a.setAttribute("target", "_self"))
      if (!win._zebPatched) {
        win._zebPatched = true
        win.open = (u: string) => { win.location.href = u; return win }
        // forward shortcuts from inside iframe to parent
        const forward = (e: KeyboardEvent) => {
          const mod = e.ctrlKey || e.metaKey
          if (mod && e.key.toLowerCase() === "l") { e.preventDefault(); window.dispatchEvent(new KeyboardEvent("keydown", {key:"l", ctrlKey:true, metaKey:e.metaKey} as any)) }
          if (mod && e.key.toLowerCase() === "r") { e.preventDefault(); win.location.reload() }
          if (e.key === "F5") { e.preventDefault(); win.location.reload() }
          if (e.altKey && e.key === "ArrowLeft") { e.preventDefault(); win.history.back() }
          if (e.altKey && e.key === "ArrowRight") { e.preventDefault(); win.history.forward() }
          if (e.key === "Escape") { window.dispatchEvent(new KeyboardEvent("keydown", {key:"Escape"} as any)) }
        }
        doc.addEventListener("keydown", forward)
      }
    } catch {}
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
        {isStart ? <div className="blank" /> : (
          <iframe
            ref={iframeRef}
            key={url}
            className="webview"
            src={url!}
            title="web"
            sandbox="allow-scripts allow-same-origin allow-forms allow-popups allow-top-navigation allow-downloads"
            allow="clipboard-read; clipboard-write; fullscreen; autoplay; encrypted-media"
            referrerPolicy="no-referrer-when-downgrade"
            onLoad={onIframeLoad}
          />
        )}
      </div>

      {!isStart && (
        <button className="home-btn" onClick={() => { setUrl(null); setInput(""); setTopVisible(false) }} title="Home">
          ⌘
        </button>
      )}
    </div>
  )
}
