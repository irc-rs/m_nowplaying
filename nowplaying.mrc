;*** FOR DEBUGGING ***
alias -l m_nowplaying.dll return $cmdline

; Usage:
;    Alias (Sync): $m_nowplaying(<procname>, [data])
;   Alias (Async): $m_nowplaying(<procname>, [data]).callback
; Command (Async): /m_nowplaying <procname> <data>
;  Command (Sync): /m_nowplaying -s <procname> <data>

on 1:START:{
  echo -at * Loaded: $m_nowplaying(version)
  noop $m_nowplaying(wait_for_media).m_nowplaying:mediachanged
}

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
alias -l m_nowplaying.dll return $+($scriptdir, m_nowplaying_x, $iif($bits == 64, x64, x86), .dll)

; Note: We have to expose this as a global alias for mIRC, while AdiIRC allows us to keep it local.
alias m_nowplaying:mediachanged {
  if (%m_nowplaying.status == stopping) {
    unset %m_nowplaying.status
  }
  else {
    .signal -n m_nowplaying media_changed
    noop $m_nowplaying(wait_for_media, data).m_nowplaying:mediachanged
  }
}

alias m_nowplaying:halt {
  set %m_nowplaying.status stopping
  echo -at [m_nowplaying] No longer listening for media updates.
  if ($m_nowplaying(halt) != S_OK) {
    echo -st [m_nowplaying] Error halting m_nowplaying
  }
}

on 1:signal:m_nowplaying:{
  echo -at * Now Playing: $+($chr(91),$chr(2),$m_nowplaying(title),$chr(2),$chr(93)) $iif($m_nowplaying(artist), by $+($chr(91),$ifmatch,$chr(93)))
}
