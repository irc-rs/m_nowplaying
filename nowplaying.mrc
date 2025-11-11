;*** FOR DEBUGGING ***
on 1:START:{
  echo -at Loaded: $dll($qt($$cmdline), version, $null)
  noop $m_nowplaying(wait_for_media, data).m_nowplaying:mediachanged
}

; Overrides (testing)
alias -l m_nowplaying.dll return $cmdline

; Usage:
;    Alias (Sync): $m_nowplaying(procname, data)
;   Alias (Async): $m_nowplaying(procname, data).callback
; Command (Async): /m_nowplaying procname data
;  Command (Sync): /m_nowplaying -s procname data

alias m_nowplaying {
  var %dll = $qt($m_nowplaying.dll)
  if ($isid) {
    if (!$1) tokenize 32 version
    if ($prop) return $dllcall(%dll, $prop, $$1, $2-)
    return $dll(%dll, $$1, $2-)
  }
  if ($1 == -s) return dll %dll $$2-
  noop $dllcall(%dll, noop, $$1, $2-)
}

; TODO: Doesn't take into account aarch64
alias -l m_nowplaying.dll return $+($scriptdir, m_nowplaying_x, $bits, .dll)

; Note: We have to expose this for mIRC, while AdiIRC allows us to keep it local.
alias m_nowplaying:mediachanged {
  echo -at Note: *** Media changed *** $+([,$m_nowplaying(title) - $m_nowplaying(artist),])
  .signal -n m_nowplaying media_changed
  noop $m_nowplaying(wait_for_media, data).m_nowplaying:mediachanged
}

alias m_nowplaying.haltall echo -at [m_nowplaying] Cancelled all asynchronous calls $m_nowplaying(halt)
